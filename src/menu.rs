//! Native menu bar (macOS).
//!
//! TinyView launches as a bare detached binary / `.app` bundle with an empty
//! menu bar, so the standard macOS shortcuts users expect — Cmd+Q (quit),
//! Ctrl+Cmd+F (fullscreen), Cmd+W (close), Cmd+M (minimize), Cmd+C/V/X/A
//! (edit) — do nothing: there is no menu to own those accelerators.
//!
//! This module is a thin builder: it walks [`crate::shortcuts::MENU_LAYOUT`]
//! (the platform-independent, unit-tested source of truth for the menu's
//! contents) and maps each [`MenuItem`] to a `muda` *predefined* item. Each
//! predefined item maps to an AppKit standard selector (`terminate:`,
//! `toggleFullScreen:`, `performClose:`, `copy:` …), so AppKit dispatches them
//! through the NSMenu / responder chain. That is why this works even while the
//! WKWebView holds first-responder focus — intercepting key events directly in
//! the tao event loop would not, because the WebView swallows the keystroke
//! before tao sees it.
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
    use crate::shortcuts::MENU_LAYOUT;
    use muda::{Menu, Submenu};

    let menu = Menu::new();
    for section in MENU_LAYOUT {
        let submenu = Submenu::new(section.title, true);
        for &item in section.items {
            // `append` only errors on duplicate-item insertion, which cannot
            // happen here: every call builds a fresh predefined item.
            let _ = submenu.append(&predefined(item));
        }
        let _ = menu.append(&submenu);
    }

    // Requires the NSApp to already exist — only call after the tao event loop
    // has been built. Must run on the main thread (it does: tao event loops are
    // main-thread only).
    menu.init_for_nsapp();

    menu
}

/// Map a layout [`MenuItem`] to its `muda` predefined item. The accelerators
/// are AppKit's macOS defaults (asserted against [`crate::shortcuts`]); we do
/// not pass our own.
#[cfg(target_os = "macos")]
fn predefined(item: crate::shortcuts::MenuItem) -> muda::PredefinedMenuItem {
    use crate::shortcuts::MenuItem;
    use muda::{AboutMetadata, PredefinedMenuItem};

    match item {
        MenuItem::About => PredefinedMenuItem::about(
            None,
            Some(AboutMetadata {
                name: Some("TinyView".to_owned()),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
                ..Default::default()
            }),
        ),
        MenuItem::Hide => PredefinedMenuItem::hide(None),
        MenuItem::HideOthers => PredefinedMenuItem::hide_others(None),
        MenuItem::ShowAll => PredefinedMenuItem::show_all(None),
        MenuItem::Quit => PredefinedMenuItem::quit(None),
        MenuItem::Cut => PredefinedMenuItem::cut(None),
        MenuItem::Copy => PredefinedMenuItem::copy(None),
        MenuItem::Paste => PredefinedMenuItem::paste(None),
        MenuItem::SelectAll => PredefinedMenuItem::select_all(None),
        MenuItem::Fullscreen => PredefinedMenuItem::fullscreen(None),
        MenuItem::Minimize => PredefinedMenuItem::minimize(None),
        MenuItem::CloseWindow => PredefinedMenuItem::close_window(None),
        MenuItem::Separator => PredefinedMenuItem::separator(),
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::predefined;
    use crate::shortcuts::{MenuItem, MENU_LAYOUT};

    // `predefined` builds a `muda::PredefinedMenuItem`. This is safe on cargo
    // test's worker thread (not the process main thread): muda defers
    // NSMenuItem allocation until the item is appended to a menu, so
    // construction needs no `MainThreadMarker`. Only `install` (which appends
    // and calls `init_for_nsapp`) requires the main thread, and that is the
    // GUI-launch path, exercised by the smoke run rather than here.

    /// Each [`MenuItem`] maps to the *intended* predefined action. The
    /// exhaustive match in [`predefined`] guarantees every variant is handled,
    /// but not that the mapping is correct — a typo like `Quit => copy(None)`
    /// compiles cleanly. We pin the mapping via each item's default label
    /// (`text()`), which muda derives from the predefined type. Labels carry an
    /// `&` mnemonic in muda's table but `text()` returns it stripped.
    #[test]
    fn each_item_maps_to_the_expected_action() {
        // (item, substring expected in the predefined item's default label)
        let cases = [
            (MenuItem::About, "About"),
            (MenuItem::Hide, "Hide"),
            (MenuItem::HideOthers, "Hide Others"),
            (MenuItem::ShowAll, "Show All"),
            (MenuItem::Quit, "Quit"),
            (MenuItem::Cut, "Cut"),
            (MenuItem::Copy, "Copy"),
            (MenuItem::Paste, "Paste"),
            (MenuItem::SelectAll, "Select All"),
            (MenuItem::Fullscreen, "Full Screen"),
            (MenuItem::Minimize, "Minimize"),
            (MenuItem::CloseWindow, "Close"),
        ];
        for (item, expected) in cases {
            let text = predefined(item).text();
            assert!(
                text.contains(expected),
                "{item:?} mapped to an item labeled {text:?}; expected it to contain {expected:?}"
            );
        }
    }

    /// Building a predefined item for every entry actually used in the layout
    /// (including `Separator` and the `About`-metadata path) must not panic.
    #[test]
    fn builds_every_layout_item() {
        for section in MENU_LAYOUT {
            for &item in section.items {
                let _ = predefined(item);
            }
        }
    }

    /// A separator is a divider, so it carries no label.
    #[test]
    fn separator_has_empty_label() {
        assert!(predefined(MenuItem::Separator).text().is_empty());
    }
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
