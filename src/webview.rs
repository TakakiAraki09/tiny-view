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
const MACOS_CLIPBOARD_NEUTRALIZE: &str = "\
try { Object.defineProperty(navigator, 'clipboard', { value: undefined, configurable: false }); } \
catch (e) { try { navigator.clipboard = undefined; } catch (_) {} }";

/// Build a `WebView` with TinyView's ephemeral defaults applied.
///
/// Defaults applied unconditionally:
///   - `with_incognito(true)` unless `perms.allow_storage`
///   - `with_devtools(false)` in release builds, `true` in debug
///   - `with_clipboard(perms.allow_clipboard)`
///   - `with_navigation_handler` rejecting non-`about:` / non-`data:` top-level navigation
///   - On macOS, an initialization script disabling `navigator.clipboard`
///     (unless `perms.allow_clipboard`)
///
/// CSP `<meta>` is injected into HTML unless `opts.raw_mode` is set AND
/// no permission flag has been granted (PRD §19.5).
pub fn build(window: &Window, opts: BuildOptions<'_>) -> wry::Result<WebView> {
    let any_perm = opts.perms.allow_fetch || opts.perms.allow_clipboard || opts.perms.allow_storage;
    let inject_csp_now = !opts.raw_mode || any_perm;

    // Avoid an allocation when we won't inject CSP.
    let html_owned: Option<String> = if inject_csp_now {
        Some(inject_csp(opts.html, &opts.perms))
    } else {
        None
    };
    let html_to_load: &str = html_owned.as_deref().unwrap_or(opts.html);

    let mut builder = WebViewBuilder::new()
        .with_html(html_to_load)
        .with_incognito(!opts.perms.allow_storage)
        .with_clipboard(opts.perms.allow_clipboard)
        .with_navigation_handler(|url: String| {
            // Top-level navigation policy: only `about:` and `data:` are allowed.
            // Everything else (http/https/file/custom schemes) is rejected.
            url.starts_with("about:") || url.starts_with("data:")
        });

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
        if !opts.perms.allow_clipboard {
            builder = builder.with_initialization_script(MACOS_CLIPBOARD_NEUTRALIZE);
        }
    }

    builder.build(window)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let meta_idx = out.find("<meta http-equiv=\"Content-Security-Policy\"").unwrap();
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
        let meta_idx = out.find("<meta http-equiv=\"Content-Security-Policy\"").unwrap();
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
}
