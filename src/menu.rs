//! Native menu bar (macOS).
//!
//! TinyView launches as a bare detached binary / `.app` bundle with an empty
//! menu bar, so the standard macOS shortcuts users expect — Cmd+Q (quit),
//! Ctrl+Cmd+F (fullscreen), Cmd+W (close), Cmd+M (minimize), Cmd+C/V/X/A
//! (edit) — do nothing: there is no menu to own those accelerators.
//!
//! We install a minimal standard menu built entirely from `muda`'s
//! *predefined* items. Each predefined item maps to an AppKit standard
//! selector (`terminate:`, `toggleFullScreen:`, `performClose:`, `copy:` …),
//! so AppKit dispatches them through the NSMenu / responder chain. That is why
//! this works even while the WKWebView holds first-responder focus —
//! intercepting key events directly in the tao event loop would not, because
//! the WebView swallows the keystroke before tao sees it.
//!
//! Because every item is a predefined AppKit action, TinyView neither listens
//! for `muda::MenuEvent` nor installs any JS↔native bridge: the security model
//! (CLAUDE.md "no native bridge") and the ephemeral guarantee are unchanged.
//!
//! macOS only. `muda` is declared as a macOS-target-only dependency, so other
//! platforms compile the no-op stub and keep their existing menu-less
//! behavior.

/// Install the standard menu bar onto the running NSApp and return its owning
/// handle.
///
/// AppKit retains its own reference to the menu via `setMainMenu:`, so the
/// installed menu survives even if this handle is dropped. We still bind it to
/// a local that outlives `event_loop.run` (which diverges with `-> !` on
/// macOS, so the local lives for the whole process): this keeps the Rust-side
/// `muda::Menu` — and the menu delegate it owns — alive for as long as the menu
/// is on screen, rather than relying on AppKit's retain alone.
///
/// On non-macOS this is a no-op returning a zero-sized guard.
#[cfg(target_os = "macos")]
pub fn install() -> muda::Menu {
    use muda::{AboutMetadata, Menu, PredefinedMenuItem, Submenu};

    let menu = Menu::new();

    // On macOS the first submenu becomes the application menu; its title is
    // replaced by the app name (from the `.app` bundle / process name).
    let app = Submenu::new("TinyView", true);
    let _ = app.append_items(&[
        &PredefinedMenuItem::about(
            None,
            Some(AboutMetadata {
                name: Some("TinyView".to_owned()),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
                ..Default::default()
            }),
        ),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::hide(None),
        &PredefinedMenuItem::hide_others(None),
        &PredefinedMenuItem::show_all(None),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::quit(None),
    ]);

    // Edit: native Cmd+C/V/X/A. PRD §19.3 documents that WKWebView clipboard
    // shortcuts are always available at the OS level — these menu items just
    // surface the standard accelerators; they add no new capability.
    let edit = Submenu::new("Edit", true);
    let _ = edit.append_items(&[
        &PredefinedMenuItem::cut(None),
        &PredefinedMenuItem::copy(None),
        &PredefinedMenuItem::paste(None),
        &PredefinedMenuItem::select_all(None),
    ]);

    // View: Enter Full Screen (Ctrl+Cmd+F via AppKit's toggleFullScreen:).
    let view = Submenu::new("View", true);
    let _ = view.append_items(&[&PredefinedMenuItem::fullscreen(None)]);

    // Window: Minimize (Cmd+M) and Close (Cmd+W). For TinyView's single
    // ephemeral window, Close behaves like Quit — closing tears down the
    // WebView and ends the process.
    let window = Submenu::new("Window", true);
    let _ = window.append_items(&[
        &PredefinedMenuItem::minimize(None),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::close_window(None),
    ]);

    let _ = menu.append_items(&[&app, &edit, &view, &window]);

    // Requires the NSApp to already exist — only call after the tao event loop
    // has been built. Must run on the main thread (it does: tao event loops are
    // main-thread only).
    menu.init_for_nsapp();

    menu
}

/// Zero-sized guard returned by the non-macOS [`install`] stub. Exists so the
/// call site (`let guard = menu::install();`) stays platform-uniform and does
/// not bind a unit value (which would trip `clippy::let_unit_value` under the
/// CI `-D warnings` on the Linux / Windows runners).
#[cfg(not(target_os = "macos"))]
pub struct MenuGuard;

/// Non-macOS stub: TinyView keeps its existing menu-less behavior.
#[cfg(not(target_os = "macos"))]
pub fn install() -> MenuGuard {
    MenuGuard
}
