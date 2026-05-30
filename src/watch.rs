//! `--watch` mode: re-render and reload the WebView when the source file changes.
//!
//! ## Design
//!
//! - Crate: `notify` + `notify-debouncer-mini` (always linked, no feature gate).
//! - Debounce: 100ms trailing — short enough to feel live, long enough to coalesce
//!   the typical "atomic save" pattern used by VSCode / Vim / IntelliJ (write to
//!   tempfile + rename).
//! - Concurrency model: notify thread → [`tao::event_loop::EventLoopProxy`]
//!   `send_event(UserEvent::Reload(html))` → event loop thread calls
//!   `webview.load_html(&html)`. We never touch the WebView from the watcher
//!   thread (wry / tao are main-thread-only on macOS).
//! - The WebView itself is not destroyed; only its HTML is replaced. Scroll
//!   position / focus / `window.__TINYVIEW__` are reset by the reload — this is
//!   intentional and documented in PRD §9.10.
//!
//! ## macOS FSEvents caveat
//!
//! macOS FSEvents report events at directory granularity. Watching the file
//! directly works on `RecursiveMode::NonRecursive` for most backends, but on
//! macOS the underlying notify backend may report events for the parent
//! directory; therefore we watch the **parent directory** and filter inside
//! the handler by comparing `event.path` against our canonicalized target.
//!
//! ## Errors
//!
//! - Failure to spawn the watcher returns the `notify::Error`. Caller should
//!   log and fall through to a non-watch launch (or exit).
//! - Failure to re-read the source file during a reload event is logged on
//!   stderr; the existing WebView is left unchanged.
//! - Failure to send the reload event (event loop exited) silently stops the
//!   watcher — the user has closed the window.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use notify_debouncer_mini::{
    new_debouncer,
    notify::{self, RecommendedWatcher, RecursiveMode},
    DebounceEventResult, Debouncer,
};
use tao::event_loop::EventLoopProxy;

use crate::template::{self, InjectData, TemplateRef};

/// Trailing debounce window. 100ms is the documented value (PRD §9.10).
const DEBOUNCE_MS: u64 = 100;

/// User event passed from the watcher thread back to the tao event loop.
///
/// `Reload(html)` carries the *fully composed* HTML (template substitution
/// already applied). CSP injection is the event loop's job because it depends
/// on `Permissions` which we don't carry here.
#[derive(Debug, Clone)]
pub enum UserEvent {
    Reload(String),
}

/// Inputs required to re-render the source on each change.
///
/// Cloned into the watcher thread once at spawn time; nothing here is mutated.
pub struct WatchContext {
    pub source: PathBuf,
    pub template: TemplateRef,
    pub params: HashMap<String, String>,
    pub raw_mode: bool,
}

/// Spawn a debounced watcher on `ctx.source`'s parent directory and forward
/// reloads to `proxy`. The returned [`Debouncer`] stops on drop.
///
/// The caller should keep the returned guard alive for the lifetime of the
/// WebView (typically by binding it in `main` before `event_loop.run(...)`).
pub fn spawn_watcher(
    ctx: WatchContext,
    proxy: EventLoopProxy<UserEvent>,
) -> notify::Result<Debouncer<RecommendedWatcher>> {
    // Canonicalize the target path so we can compare against event paths
    // (which on macOS FSEvents may be reported as the resolved real path).
    // If canonicalization fails (file moved/removed mid-startup), fall back
    // to the raw path — a future event will re-canonicalize implicitly via
    // the filesystem.
    let target = std::fs::canonicalize(&ctx.source).unwrap_or_else(|_| ctx.source.clone());

    // Determine which directory to watch. macOS FSEvents emit at the parent
    // directory anyway; this also keeps us functional across atomic-save
    // rename(2) which removes-and-recreates the file.
    let watch_dir = target
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let handler_ctx = HandlerContext {
        target,
        source_for_render: ctx.source.clone(),
        template: ctx.template,
        params: ctx.params,
        raw_mode: ctx.raw_mode,
        proxy,
    };

    let mut debouncer = new_debouncer(
        Duration::from_millis(DEBOUNCE_MS),
        move |result: DebounceEventResult| handle_events(&handler_ctx, result),
    )?;

    debouncer
        .watcher()
        .watch(&watch_dir, RecursiveMode::NonRecursive)?;

    Ok(debouncer)
}

/// Captured-by-move state for the watcher callback.
struct HandlerContext {
    /// Canonicalized target path used for equality checks against event paths.
    target: PathBuf,
    /// Raw path (may be relative) used when re-reading the source — keeps
    /// behavior consistent with the initial read in `main::read_input`.
    source_for_render: PathBuf,
    template: TemplateRef,
    params: HashMap<String, String>,
    raw_mode: bool,
    proxy: EventLoopProxy<UserEvent>,
}

fn handle_events(ctx: &HandlerContext, result: DebounceEventResult) {
    let events = match result {
        Ok(ev) => ev,
        Err(e) => {
            eprintln!("tinyview: watch error: {e}");
            return;
        }
    };

    // Filter: only fire when at least one event references our target file.
    // macOS FSEvents may include the parent directory or sibling files when
    // we're watching the parent dir; we must match exact paths.
    let touches_target = events.iter().any(|e| paths_match(&e.path, &ctx.target));
    if !touches_target {
        return;
    }

    let html = match recompose_html(ctx) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("tinyview: watch reload failed: {e}");
            return;
        }
    };

    // EventLoopClosed => the user closed the window. Nothing we can do, and
    // we want the watcher thread to wind down naturally on next drop.
    if ctx.proxy.send_event(UserEvent::Reload(html)).is_err() {
        // Silent: this is the expected shutdown path.
    }
}

/// Path equality that tolerates differences in canonicalization between the
/// path we recorded at spawn and the path notify reports. We try a direct
/// equality first (cheap), then a canonicalized comparison (handles symlinks
/// / `..` segments / case-folded macOS paths).
fn paths_match(event_path: &Path, target: &Path) -> bool {
    if event_path == target {
        return true;
    }
    match std::fs::canonicalize(event_path) {
        Ok(canon) => canon == *target,
        Err(_) => false,
    }
}

/// Re-read the source file and re-render through the same template pipeline
/// used for the initial render in `main::run`.
fn recompose_html(ctx: &HandlerContext) -> std::io::Result<String> {
    let input = std::fs::read_to_string(&ctx.source_for_render)?;

    if ctx.raw_mode {
        // PRD §13.1: raw skips template substitution entirely.
        return Ok(input);
    }

    let data = InjectData {
        input: &input,
        params: &ctx.params,
        title: "tinyview",
        path: Some(ctx.source_for_render.as_path()),
    };
    template::render(&ctx.template, &data).map_err(|e| {
        // RenderError → io::Error so the caller has a single error type.
        std::io::Error::other(e.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_match_direct_equality() {
        let p = PathBuf::from("/tmp/some-file.html");
        assert!(paths_match(&p, &p));
    }

    #[test]
    fn paths_match_canonicalizes() {
        // Create a tempfile, then ask paths_match to compare a path with a
        // `.` segment against the canonicalized version. This exercises the
        // canonicalize fallback branch.
        let dir = std::env::temp_dir();
        let path = dir.join("tinyview_watch_paths_match.html");
        std::fs::write(&path, b"x").expect("write tmp");

        let canon = std::fs::canonicalize(&path).expect("canonicalize");
        // Build a path with a redundant `.` segment so equality fails but
        // canonicalize collapses it.
        let weird = dir.join(".").join("tinyview_watch_paths_match.html");
        assert!(paths_match(&weird, &canon));

        let _ = std::fs::remove_file(&path);
    }

    // Note: `recompose_html` cannot easily be exercised in a unit test because
    // `HandlerContext` carries an `EventLoopProxy`, and tao's event loop must
    // be built on the main thread. The function is exercised end-to-end via
    // manual `--watch` runs (see PRD §9.10) and via integration in main.rs.
}
