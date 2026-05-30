//! Declarative menu / keyboard-shortcut layout.
//!
//! This is the single source of truth for *what* the native menu bar contains.
//! It is pure data — no `muda`, no AppKit, no NSApp — so it can be unit-tested
//! without building the real menu or opening a window. The macOS menu builder
//! ([`crate::menu`]) walks [`MENU_LAYOUT`] and maps each [`MenuItem`] to a
//! `muda` predefined item; it adds no items of its own, so this table fully
//! describes the menu.
//!
//! Compiled on macOS only (the menu is macOS-only — see [`crate::menu`]); other
//! platforms keep their existing menu-less behavior. The tests run on the macOS
//! CI runner.
//!
//! Accelerators here are the macOS defaults that `muda`'s predefined items
//! attach automatically (we never pass them to `muda`). They are kept as
//! documentation / regression-test expectations — `accelerator()` is the spec
//! the on-screen shortcuts are checked against.

/// A standard menu entry. Every variant maps to an AppKit / `muda` predefined
/// item (see [`crate::menu`]); [`MenuItem::Separator`] is a visual divider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuItem {
    About,
    Hide,
    HideOthers,
    ShowAll,
    Quit,
    Cut,
    Copy,
    Paste,
    SelectAll,
    Fullscreen,
    Minimize,
    CloseWindow,
    Separator,
}

impl MenuItem {
    /// Expected default macOS accelerator, in `muda`/`tao` notation
    /// (`"Cmd+Q"`, `"Ctrl+Cmd+F"`). `None` for items AppKit leaves without a
    /// shortcut ([`MenuItem::About`], [`MenuItem::ShowAll`]) and for
    /// [`MenuItem::Separator`].
    ///
    /// This is the *expected* spec, not a value we pass to `muda` (predefined
    /// items carry AppKit's defaults). It is consumed only by the tests that
    /// guard the shortcut layout, hence dead outside `cfg(test)`.
    #[cfg_attr(not(test), expect(dead_code))]
    pub const fn accelerator(self) -> Option<&'static str> {
        match self {
            MenuItem::Hide => Some("Cmd+H"),
            MenuItem::HideOthers => Some("Cmd+Alt+H"),
            MenuItem::Quit => Some("Cmd+Q"),
            MenuItem::Cut => Some("Cmd+X"),
            MenuItem::Copy => Some("Cmd+C"),
            MenuItem::Paste => Some("Cmd+V"),
            MenuItem::SelectAll => Some("Cmd+A"),
            MenuItem::Fullscreen => Some("Ctrl+Cmd+F"),
            MenuItem::Minimize => Some("Cmd+M"),
            MenuItem::CloseWindow => Some("Cmd+W"),
            MenuItem::About | MenuItem::ShowAll | MenuItem::Separator => None,
        }
    }

    /// Whether this entry is a visual divider rather than an actionable item.
    /// Used only by the layout tests.
    #[cfg_attr(not(test), expect(dead_code))]
    pub const fn is_separator(self) -> bool {
        matches!(self, MenuItem::Separator)
    }
}

/// One top-level menu (a submenu of the menu bar).
pub struct MenuSection {
    /// Display title. On macOS the *first* section's title is replaced by the
    /// application name, so it conventionally becomes the app menu.
    pub title: &'static str,
    pub items: &'static [MenuItem],
}

use MenuItem::*;

/// The full menu-bar layout. Order matters: on macOS the first section is the
/// application menu. Standard macOS arrangement — App / Edit / View / Window.
pub const MENU_LAYOUT: &[MenuSection] = &[
    MenuSection {
        title: "TinyView",
        items: &[About, Separator, Hide, HideOthers, ShowAll, Separator, Quit],
    },
    MenuSection {
        title: "Edit",
        items: &[Cut, Copy, Paste, SelectAll],
    },
    MenuSection {
        title: "View",
        items: &[Fullscreen],
    },
    MenuSection {
        title: "Window",
        items: &[Minimize, Separator, CloseWindow],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    /// The shortcuts the user explicitly asked for must be present and carry
    /// their standard macOS accelerators.
    #[test]
    fn required_shortcuts_present_with_expected_accelerators() {
        let expected = [
            (Quit, "Cmd+Q"),
            (Fullscreen, "Ctrl+Cmd+F"),
            (CloseWindow, "Cmd+W"),
            (Minimize, "Cmd+M"),
            (Copy, "Cmd+C"),
            (Paste, "Cmd+V"),
        ];
        for (item, accel) in expected {
            assert!(
                layout_contains(item),
                "{item:?} must appear in the menu layout"
            );
            assert_eq!(
                item.accelerator(),
                Some(accel),
                "{item:?} should map to {accel}"
            );
        }
    }

    /// On macOS the first section becomes the application menu, so Quit lives
    /// there by convention.
    #[test]
    fn first_section_is_app_menu_and_holds_quit() {
        let app = &MENU_LAYOUT[0];
        assert_eq!(app.title, "TinyView");
        assert!(app.items.contains(&Quit), "Quit belongs in the app menu");
    }

    /// Separators are dividers between items — they must never sit at the edge
    /// of a section or appear back-to-back (that renders as a doubled line).
    #[test]
    fn separators_are_never_dangling_or_doubled() {
        for section in MENU_LAYOUT {
            let items = section.items;
            assert!(
                !items.first().is_some_and(|i| i.is_separator()),
                "{}: must not start with a separator",
                section.title
            );
            assert!(
                !items.last().is_some_and(|i| i.is_separator()),
                "{}: must not end with a separator",
                section.title
            );
            for pair in items.windows(2) {
                assert!(
                    !(pair[0].is_separator() && pair[1].is_separator()),
                    "{}: has two consecutive separators",
                    section.title
                );
            }
        }
    }

    /// No two actionable items may share an accelerator, or one shortcut would
    /// shadow the other.
    #[test]
    fn accelerators_are_unique() {
        let mut seen: Vec<&str> = Vec::new();
        for section in MENU_LAYOUT {
            for item in section.items {
                if let Some(accel) = item.accelerator() {
                    assert!(
                        !seen.contains(&accel),
                        "accelerator {accel} is assigned to more than one item"
                    );
                    seen.push(accel);
                }
            }
        }
    }

    /// Structural sanity: every section is titled and non-empty.
    #[test]
    fn every_section_is_titled_and_non_empty() {
        for section in MENU_LAYOUT {
            assert!(!section.title.is_empty(), "section title must not be empty");
            assert!(
                !section.items.is_empty(),
                "{}: section must have at least one item",
                section.title
            );
        }
    }

    /// A separator carries no accelerator (it is not actionable).
    #[test]
    fn separator_has_no_accelerator() {
        assert!(Separator.is_separator());
        assert_eq!(Separator.accelerator(), None);
    }

    /// Lock the accelerator spec to the variant set: every actionable layout
    /// item carries an accelerator, except the two AppKit leaves without a
    /// default shortcut (About / Show All). Catches a new variant being added
    /// to the layout without deciding its accelerator (or vice versa).
    #[test]
    fn actionable_items_have_accelerators_except_about_and_show_all() {
        for section in MENU_LAYOUT {
            for &item in section.items {
                if item.is_separator() {
                    continue;
                }
                let has_accel = item.accelerator().is_some();
                let expected = !matches!(item, About | ShowAll);
                assert_eq!(
                    has_accel, expected,
                    "{item:?}: accelerator presence does not match the spec"
                );
            }
        }
    }

    fn layout_contains(target: MenuItem) -> bool {
        MENU_LAYOUT
            .iter()
            .flat_map(|s| s.items.iter())
            .any(|&i| i == target)
    }
}
