//! Ephemeral-by-default WebView builder wrapper.
//!
//! See PRD §19 (Security Model) for the design rationale. This module
//! applies ephemeral defaults (`with_incognito(true)`, `with_devtools`
//! off in release, `with_clipboard(false)`, navigation handler that
//! rejects top-level external navigation) and injects a restrictive
//! Content-Security-Policy `<meta>` tag into the HTML.
//!
//! `raw_mode` skips CSP injection only — WebView builder defaults
//! still apply (PRD §19.5).

use tao::window::Window;
use wry::{WebView, WebViewBuilder};

/// User-controlled permission relaxations.
///
/// Each flag relaxes exactly one slice of the default-deny policy.
#[derive(Debug, Clone, Copy, Default)]
pub struct Permissions {
    /// Relax CSP `connect-src` from `'none'` to `https: http: ws: wss:`.
    pub allow_fetch: bool,
    /// Pass `with_clipboard(true)` to wry. macOS is always-on at the OS
    /// level and cannot be fully disabled; see PRD §19.3.
    pub allow_clipboard: bool,
    /// Pass `with_incognito(false)` to wry. Without this, storage is
    /// purged when the WebView is dropped.
    pub allow_storage: bool,
}

/// Options for [`build`].
pub struct BuildOptions<'a> {
    /// Single HTML string to render. The wrapper may inject a CSP
    /// `<meta>` tag into the `<head>` (unless `raw_mode` is set).
    pub html: &'a str,
    pub perms: Permissions,
    /// Raw mode skips CSP `<meta>` injection (PRD §19.5). WebView
    /// builder defaults are still applied. `--allow-*` flags force CSP
    /// injection regardless of this flag.
    pub raw_mode: bool,
    /// PRD §9.9: enable per-pixel transparency on the WebView. Must be
    /// paired with `WindowBuilder::with_transparent(true)` on the host
    /// window — otherwise the OS compositor draws an opaque background
    /// behind the WebView and the alpha channel is invisible.
    ///
    /// On macOS this requires the `transparent` wry feature (enabled in
    /// Cargo.toml) because the implementation calls a WKWebView private
    /// API (`_drawsBackground`).
    pub transparent: bool,

    /// E2E-only IPC channel for the self-test harness (issue #5). When `Some`,
    /// [`build`] installs a wry IPC handler that forwards
    /// `window.ipc.postMessage(<string>)` bodies from page JS to this channel,
    /// letting the harness observe in-page behavior (fetch / clipboard /
    /// storage). The field — and the bridge it enables — only exist under the
    /// `e2e` feature, so the production binary never carries a JS→native bridge
    /// (CLAUDE.md security default: no native bridge). Production callers do not
    /// (and cannot) set this.
    #[cfg(feature = "e2e")]
    pub ipc_tx: Option<std::sync::mpsc::Sender<String>>,
}

/// Build the default CSP value, honoring `allow_fetch`.
///
/// PRD §19.3 baseline:
///   `default-src 'self' 'unsafe-inline' data: blob:;
///    connect-src 'none';
///    object-src 'none';
///    base-uri 'none';
///    form-action 'none';`
pub fn build_csp(perms: &Permissions) -> String {
    let connect_src = if perms.allow_fetch {
        "connect-src https: http: ws: wss:"
    } else {
        "connect-src 'none'"
    };
    format!(
        "default-src 'self' 'unsafe-inline' data: blob:; \
         {connect_src}; \
         object-src 'none'; \
         base-uri 'none'; \
         form-action 'none';"
    )
}

/// Extract the value of `attr` from a (lowercased) tag fragment.
///
/// Handles both single- and double-quoted values (e.g. `content="fetch"` and
/// `content='fetch'`). Returns the first match. Intentionally simple — we avoid
/// pulling in `regex`/an HTML parser for startup-time reasons (CLAUDE.md KPI #1).
fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let key = format!("{attr}=");
    let mut from = 0;
    while let Some(rel) = tag[from..].find(&key) {
        let vstart = from + rel + key.len();
        let bytes = tag.as_bytes();
        if vstart >= bytes.len() {
            return None;
        }
        let quote = bytes[vstart];
        if quote == b'"' || quote == b'\'' {
            let rest = &tag[vstart + 1..];
            if let Some(end) = rest.find(quote as char) {
                return Some(rest[..end].to_string());
            }
        }
        // Not a quoted value we understand; keep scanning for another `attr=`.
        from = vstart;
    }
    None
}

/// Scan the HTML for a `<meta name="tinyview-allow" content="...">` tag and
/// return true when the space-separated `content` token list includes `fetch`.
///
/// This lets a template/document opt into outbound fetch (XHR / WebSocket)
/// without the CLI `--allow-fetch` flag; the two are OR'd (see [`effective_perms`]).
/// The trust model is that TinyView only renders content the user themselves
/// pipes in, so a self-declared `<meta>` is an expression of the user's intent
/// (PRD §19). Only `fetch` is meta-grantable today; clipboard/storage remain
/// CLI-only.
///
/// Like the rest of this module we use a deliberately simple string scan rather
/// than a real HTML parser (CLAUDE.md KPI #1: startup time).
fn meta_allows_fetch(html: &str) -> bool {
    let lower = html.to_ascii_lowercase();
    let mut search_from = 0;
    while let Some(rel) = lower[search_from..].find("<meta") {
        let tag_start = search_from + rel;
        let tag_end = match lower[tag_start..].find('>') {
            Some(e) => tag_start + e,
            None => break,
        };
        let tag = &lower[tag_start..tag_end];
        let is_allow_tag = tag.contains("name=\"tinyview-allow\"")
            || tag.contains("name='tinyview-allow'");
        if is_allow_tag {
            if let Some(content) = extract_attr(tag, "content") {
                if content.split_whitespace().any(|t| t == "fetch") {
                    return true;
                }
            }
        }
        search_from = tag_end + 1;
    }
    false
}

/// Combine CLI-granted permissions with permissions declared inline in the HTML
/// via `<meta name="tinyview-allow" ...>`. Currently only `fetch` is
/// meta-grantable, and it is OR'd with the `--allow-fetch` flag (whichever
/// grants it wins). Clipboard/storage are unaffected (CLI-only).
fn effective_perms(html: &str, perms: &Permissions) -> Permissions {
    let mut eff = *perms;
    if !eff.allow_fetch && meta_allows_fetch(html) {
        eff.allow_fetch = true;
    }
    eff
}

/// Find the start of the `<head ...>` opening tag (case-insensitive)
/// and return the byte offset *just after* the closing `>` of that tag.
fn find_head_open_end(html: &str) -> Option<usize> {
    // Case-insensitive search for "<head". We avoid pulling in `regex`
    // for startup-time reasons (CLAUDE.md KPI #1).
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<head")?;
    // After `<head`, find the next `>` that closes the opening tag.
    // Note: this is a deliberately simple parser; a robust HTML parser
    // would handle attribute strings containing '>'. For our use case
    // (template authors writing well-formed HTML) this is sufficient.
    let close_rel = lower[start..].find('>')?;
    Some(start + close_rel + 1)
}

/// Inject a CSP `<meta http-equiv>` tag into the HTML. Behavior:
/// - If `<head>` exists, insert the meta tag right after the opening tag.
/// - Otherwise, prepend a fresh `<head>...</head>` block to the document.
pub fn inject_csp(html: &str, perms: &Permissions) -> String {
    let csp = build_csp(perms);
    let meta = format!(r#"<meta http-equiv="Content-Security-Policy" content="{csp}">"#);

    match find_head_open_end(html) {
        Some(idx) => {
            let mut out = String::with_capacity(html.len() + meta.len());
            out.push_str(&html[..idx]);
            out.push_str(&meta);
            out.push_str(&html[idx..]);
            out
        }
        None => {
            let mut out = String::with_capacity(html.len() + meta.len() + 13);
            out.push_str("<head>");
            out.push_str(&meta);
            out.push_str("</head>");
            out.push_str(html);
            out
        }
    }
}

/// macOS-only initialization script that defangs `navigator.clipboard`.
///
/// PRD §19.3: WKWebView always enables clipboard at the OS level and
/// wry's `with_clipboard(false)` does nothing on macOS. We strip the
/// JS-visible `navigator.clipboard` so scripts cannot read/write the
/// clipboard programmatically. Cmd+C / Cmd+V via the native menu are
/// out of scope and remain enabled (documented constraint).
#[cfg(target_os = "macos")]
const MACOS_CLIPBOARD_NEUTRALIZE: &str = "\
try { Object.defineProperty(navigator, 'clipboard', { value: undefined, configurable: false }); } \
catch (e) { try { navigator.clipboard = undefined; } catch (_) {} }";

/// Initialization script enabling Cmd/Ctrl +/-/0 zoom.
///
/// wry 0.55 exposes no native zoom API, so zoom is implemented purely in JS by
/// driving `document.documentElement.style.zoom` (supported by WKWebView,
/// WebView2 and WebKitGTK). Injected as a WebView user script — which runs
/// before page scripts and is not subject to the page's CSP `<meta>` — so it
/// works regardless of the document's CSP, and `element.style` (CSSOM) writes
/// are not governed by `style-src` either. State lives only in the closure and
/// dies with the window, preserving the ephemeral runtime guarantee.
///
/// Bindings: Cmd (macOS) or Ctrl (Win/Linux) + `+`/`=` zoom in, `-`/`_` zoom
/// out, `0` reset to 1.0. Factor is clamped to [0.3, 5.0] in 0.1 steps.
const ZOOM_SCRIPT: &str = "\
(function () { \
  var z = 1, MIN = 0.3, MAX = 5, STEP = 0.1; \
  function round(v) { return Math.round(v * 100) / 100; } \
  function apply() { var el = document.documentElement; if (el) { el.style.zoom = String(z); } } \
  window.addEventListener('keydown', function (e) { \
    if (!(e.metaKey || e.ctrlKey)) return; \
    var k = e.key; \
    if (k === '+' || k === '=') { e.preventDefault(); z = round(Math.min(MAX, z + STEP)); apply(); } \
    else if (k === '-' || k === '_') { e.preventDefault(); z = round(Math.max(MIN, z - STEP)); apply(); } \
    else if (k === '0') { e.preventDefault(); z = 1; apply(); } \
  }, true); \
})();";

/// Apply the same HTML transformation that [`build`] performs before passing
/// the document to wry. Used by `--watch` reloads (PRD §9.10) so the new HTML
/// has the same CSP `<meta>` injected as the initial render.
///
/// Mirrors the `inject_csp_now` rule in [`build`]: CSP is injected unless
/// `raw_mode` is set AND no permission has been granted (by flag *or* by an
/// inline `<meta name="tinyview-allow">` — see [`effective_perms`]).
pub fn prepare_html(html: &str, perms: &Permissions, raw_mode: bool) -> String {
    let perms = effective_perms(html, perms);
    let any_perm = perms.allow_fetch || perms.allow_clipboard || perms.allow_storage;
    let inject = !raw_mode || any_perm;
    if inject {
        inject_csp(html, &perms)
    } else {
        html.to_string()
    }
}

/// Build a `WebView` with TinyView's ephemeral defaults applied.
///
/// Defaults applied unconditionally:
///   - `with_incognito(true)` unless `perms.allow_storage`
///   - `with_devtools(false)` in release builds, `true` in debug
///   - `with_clipboard(perms.allow_clipboard)`
///   - `with_navigation_handler` rejecting non-`about:` / non-`data:` top-level navigation
///   - On macOS, an initialization script disabling `navigator.clipboard`
///     (unless `perms.allow_clipboard`)
///   - An initialization script enabling Cmd/Ctrl +/-/0 zoom (all platforms)
///
/// CSP `<meta>` is injected into HTML unless `opts.raw_mode` is set AND
/// no permission flag has been granted (PRD §19.5).
pub fn build(window: &Window, opts: BuildOptions<'_>) -> wry::Result<WebView> {
    // Fold any inline `<meta name="tinyview-allow">` grant into the CLI perms
    // before deciding CSP/injection (PRD §19; OR semantics).
    let perms = effective_perms(opts.html, &opts.perms);
    let any_perm = perms.allow_fetch || perms.allow_clipboard || perms.allow_storage;
    let inject_csp_now = !opts.raw_mode || any_perm;

    // Avoid an allocation when we won't inject CSP.
    let html_owned: Option<String> = if inject_csp_now {
        Some(inject_csp(opts.html, &perms))
    } else {
        None
    };
    let html_to_load: &str = html_owned.as_deref().unwrap_or(opts.html);

    let mut builder = WebViewBuilder::new()
        .with_html(html_to_load)
        .with_incognito(!perms.allow_storage)
        .with_clipboard(perms.allow_clipboard)
        .with_transparent(opts.transparent)
        .with_navigation_handler(|url: String| {
            // Top-level navigation policy: only `about:` and `data:` are allowed.
            // Everything else (http/https/file/custom schemes) is rejected.
            url.starts_with("about:") || url.starts_with("data:")
        });

    // E2E self-test bridge (issue #5). Only compiled under the `e2e` feature;
    // production builds never reach this and never expose `window.ipc`.
    #[cfg(feature = "e2e")]
    if let Some(tx) = opts.ipc_tx {
        builder = builder.with_ipc_handler(move |req: wry::http::Request<String>| {
            let _ = tx.send(req.into_body());
        });
    }

    // Devtools: ON in debug builds, OFF in release. PRD §19.3.
    #[cfg(debug_assertions)]
    {
        builder = builder.with_devtools(true);
    }
    #[cfg(not(debug_assertions))]
    {
        builder = builder.with_devtools(false);
    }

    // macOS clipboard reinforcement (PRD §19.3).
    #[cfg(target_os = "macos")]
    {
        if !perms.allow_clipboard {
            builder = builder.with_initialization_script(MACOS_CLIPBOARD_NEUTRALIZE);
        }
    }

    // Cmd/Ctrl +/-/0 zoom (all platforms). Stacks on any earlier
    // initialization script; wry accumulates user scripts across calls.
    builder = builder.with_initialization_script(ZOOM_SCRIPT);

    builder.build(window)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dev-only: emit the fully-composed built-in templates (library inlined,
    /// data injected, CSP `<meta>` applied exactly as the WebView would see)
    /// to the temp dir so they can be loaded in a CSP-enforcing browser to
    /// verify the optional templates actually render under `connect-src 'none'`
    /// and no `'unsafe-eval'`. Ignored by default; run explicitly:
    ///   cargo test --release write_builtin_render_fixtures -- --ignored --nocapture
    #[test]
    #[ignore]
    fn write_builtin_render_fixtures() {
        use crate::template::{self, InjectData, TemplateRef};
        use std::collections::HashMap;

        struct Case {
            name: &'static str,
            tpl: TemplateRef,
            input: &'static str,
            params: &'static [(&'static str, &'static str)],
        }

        let dir = std::env::temp_dir();
        let cases = [
            Case {
                name: "markdown",
                tpl: TemplateRef::Markdown,
                input: "# Title\n\nSome **bold** text.\n\n```rust\nfn main() { println!(\"hi\"); }\n```\n",
                params: &[],
            },
            Case {
                name: "code",
                tpl: TemplateRef::Code,
                input: "fn main() {\n    let x = 41 + 1;\n    println!(\"{x}\");\n}\n",
                params: &[("lang", "rust")],
            },
            Case {
                name: "mermaid",
                tpl: TemplateRef::Mermaid,
                input: "graph TD; A[Start] --> B{Choice}; B -->|yes| C[OK]; B -->|no| D[Stop];",
                params: &[],
            },
        ];

        for case in &cases {
            let mut p: HashMap<String, String> = HashMap::new();
            for (k, v) in case.params {
                p.insert((*k).to_string(), (*v).to_string());
            }
            let data = InjectData {
                input: case.input,
                params: &p,
                title: "tinyview",
                path: None,
            };
            let html = template::render(&case.tpl, &data).expect("render ok");
            let prepared = prepare_html(&html, &Permissions::default(), false);
            let out = dir.join(format!("tinyview_fixture_{}.html", case.name));
            std::fs::write(&out, prepared).expect("write fixture");
            println!("FIXTURE {}: {}", case.name, out.display());
        }
    }

    #[test]
    fn csp_default_blocks_connect() {
        let csp = build_csp(&Permissions::default());
        assert!(csp.contains("connect-src 'none'"));
        assert!(csp.contains("default-src 'self' 'unsafe-inline' data: blob:"));
        assert!(csp.contains("object-src 'none'"));
        assert!(csp.contains("base-uri 'none'"));
        assert!(csp.contains("form-action 'none'"));
    }

    #[test]
    fn csp_allow_fetch_opens_connect() {
        let csp = build_csp(&Permissions {
            allow_fetch: true,
            ..Default::default()
        });
        assert!(csp.contains("connect-src https: http: ws: wss:"));
        assert!(!csp.contains("connect-src 'none'"));
    }

    #[test]
    fn inject_csp_inserts_after_head() {
        let html = "<html><head><title>x</title></head><body>y</body></html>";
        let out = inject_csp(html, &Permissions::default());
        // <meta> should appear immediately after `<head>` and before `<title>`.
        let meta_idx = out
            .find("<meta http-equiv=\"Content-Security-Policy\"")
            .unwrap();
        let title_idx = out.find("<title>").unwrap();
        let head_open_end = out.find("<head>").unwrap() + "<head>".len();
        assert_eq!(meta_idx, head_open_end);
        assert!(meta_idx < title_idx);
    }

    #[test]
    fn inject_csp_handles_head_with_attributes() {
        let html = r#"<html><head lang="en"><title>x</title></head><body></body></html>"#;
        let out = inject_csp(html, &Permissions::default());
        let head_open = out.find("<head ").unwrap();
        let head_close = out[head_open..].find('>').unwrap() + head_open + 1;
        let meta_idx = out
            .find("<meta http-equiv=\"Content-Security-Policy\"")
            .unwrap();
        assert_eq!(
            meta_idx, head_close,
            "meta must be inserted immediately after the opening <head ...> tag"
        );
    }

    #[test]
    fn inject_csp_creates_head_when_missing() {
        let html = "<html><body>only body</body></html>";
        let out = inject_csp(html, &Permissions::default());
        assert!(out.starts_with("<head>"));
        assert!(out.contains("<meta http-equiv=\"Content-Security-Policy\""));
        assert!(out.contains("</head><html>"));
    }

    #[test]
    fn inject_csp_respects_allow_fetch() {
        let html = "<head></head>";
        let out = inject_csp(
            html,
            &Permissions {
                allow_fetch: true,
                ..Default::default()
            },
        );
        assert!(out.contains("connect-src https: http: ws: wss:"));
    }

    #[test]
    fn meta_allows_fetch_detects_double_quoted() {
        let html = r#"<head><meta name="tinyview-allow" content="fetch"></head>"#;
        assert!(meta_allows_fetch(html));
    }

    #[test]
    fn meta_allows_fetch_detects_single_quoted() {
        let html = r#"<head><meta name='tinyview-allow' content='fetch'></head>"#;
        assert!(meta_allows_fetch(html));
    }

    #[test]
    fn meta_allows_fetch_is_case_insensitive() {
        let html = r#"<HEAD><META NAME="tinyview-allow" CONTENT="fetch"></HEAD>"#;
        assert!(meta_allows_fetch(html));
    }

    #[test]
    fn meta_allows_fetch_detects_token_among_others() {
        // `content` is a space-separated token list; `fetch` need not be alone.
        let html = r#"<meta name="tinyview-allow" content="clipboard fetch storage">"#;
        assert!(meta_allows_fetch(html));
    }

    #[test]
    fn meta_allows_fetch_ignores_other_names() {
        let html = r#"<meta name="description" content="fetch">"#;
        assert!(!meta_allows_fetch(html));
    }

    #[test]
    fn meta_allows_fetch_ignores_substring_tokens() {
        // `prefetch` / `fetchall` must not be mistaken for the `fetch` token.
        let html = r#"<meta name="tinyview-allow" content="prefetch fetchall">"#;
        assert!(!meta_allows_fetch(html));
    }

    #[test]
    fn meta_allows_fetch_false_when_absent() {
        let html = "<head><title>x</title></head><body>no meta here</body>";
        assert!(!meta_allows_fetch(html));
    }

    #[test]
    fn effective_perms_ors_meta_with_flag() {
        let html = r#"<meta name="tinyview-allow" content="fetch">"#;
        // CLI flag off, meta on -> fetch granted.
        let eff = effective_perms(html, &Permissions::default());
        assert!(eff.allow_fetch);

        // CLI flag on, no meta -> still granted (OR).
        let eff = effective_perms(
            "<head></head>",
            &Permissions {
                allow_fetch: true,
                ..Default::default()
            },
        );
        assert!(eff.allow_fetch);
    }

    #[test]
    fn effective_perms_does_not_touch_clipboard_or_storage() {
        let html = r#"<meta name="tinyview-allow" content="fetch clipboard storage">"#;
        // Only `fetch` is meta-grantable; clipboard/storage stay CLI-only.
        let eff = effective_perms(html, &Permissions::default());
        assert!(eff.allow_fetch);
        assert!(!eff.allow_clipboard);
        assert!(!eff.allow_storage);
    }

    #[test]
    fn prepare_html_meta_opens_connect_src() {
        let html = r#"<head><meta name="tinyview-allow" content="fetch"></head>"#;
        let out = prepare_html(html, &Permissions::default(), false);
        assert!(out.contains("connect-src https: http: ws: wss:"));
        assert!(!out.contains("connect-src 'none'"));
    }

    #[test]
    fn prepare_html_meta_forces_injection_in_raw_mode() {
        // raw_mode normally skips CSP injection, but a meta grant forces it so
        // the connect-src relaxation actually takes effect.
        let html = r#"<head><meta name="tinyview-allow" content="fetch"></head>"#;
        let out = prepare_html(html, &Permissions::default(), true);
        assert!(out.contains("<meta http-equiv=\"Content-Security-Policy\""));
        assert!(out.contains("connect-src https: http: ws: wss:"));
    }

    #[test]
    fn prepare_html_raw_mode_without_grant_skips_injection() {
        let html = "<head></head>";
        let out = prepare_html(html, &Permissions::default(), true);
        assert!(!out.contains("Content-Security-Policy"));
    }
}
