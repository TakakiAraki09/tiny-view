//! Template runtime for TinyView.
//!
//! Responsibilities:
//! - Resolve which template to use based on the priority chain
//!   (explicit > extension mapping > default_template > raw).
//! - Render a template by substituting a single marker
//!   `/*__TINYVIEW__*/ null /*__TINYVIEW__*/` with a `window.__TINYVIEW__`
//!   JSON literal.
//!
//! This module intentionally has no dependency on a `Config` type; resolution
//! inputs are passed as plain references so the resolver remains decoupled and
//! easy to test.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Marker string that runtime substitutes in template HTML.
///
/// PRD §14.1: a template includes
/// `window.__TINYVIEW__ = /*__TINYVIEW__*/ null /*__TINYVIEW__*/;`
/// and runtime replaces the marker with a JSON literal.
const MARKER: &str = "/*__TINYVIEW__*/ null /*__TINYVIEW__*/";

/// Built-in template HTML embedded at compile-time.
const TEXT_HTML: &str = include_str!("templates/text.html");
const MINIMAL_HTML: &str = include_str!("templates/minimal.html");

/// Optional built-in templates (PRD §15). These are self-contained HTML shells
/// with `/*__TINYVIEW_<LIB>__*/` placeholders that runtime replaces with the
/// vendored library JS/CSS below. The composition happens only when the
/// template is actually used, so it never touches the `raw` fast path (KPI #1).
const MARKDOWN_HTML: &str = include_str!("templates/markdown.html");
const CODE_HTML: &str = include_str!("templates/code.html");
const MERMAID_HTML: &str = include_str!("templates/mermaid.html");

/// Vendored third-party libraries inlined into the optional templates.
/// See `src/templates/vendor/README.md` for provenance and licenses. Nothing
/// here is fetched at runtime — this keeps the No Server / self-contained
/// contract (PRD §14) intact.
const MARKED_JS: &str = include_str!("templates/vendor/marked.min.js");
const HLJS_JS: &str = include_str!("templates/vendor/highlight.min.js");
const HLJS_CSS: &str = include_str!("templates/vendor/hljs-theme.css");
const MERMAID_JS: &str = include_str!("templates/vendor/mermaid.min.js");

/// Library placeholders embedded in the optional template HTML shells.
const LIB_MARKED: &str = "/*__TINYVIEW_MARKED__*/";
const LIB_HLJS: &str = "/*__TINYVIEW_HLJS__*/";
const LIB_HLJS_CSS: &str = "/*__TINYVIEW_HLJS_CSS__*/";
const LIB_MERMAID: &str = "/*__TINYVIEW_MERMAID__*/";

/// Resolved template selection.
///
/// `Raw` is treated as the fastest path: caller is expected to skip marker
/// substitution entirely (see PRD §13.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateRef {
    /// Pass the input straight to WebView. Substitution skipped.
    Raw,
    /// Built-in `text` template (plain text via `<pre>` + textContent).
    Text,
    /// Built-in `minimal` template (centered `<main>` + innerHTML).
    Minimal,
    /// Optional built-in `markdown` template (marked + highlight.js inline).
    Markdown,
    /// Optional built-in `mermaid` template (mermaid.js inline).
    Mermaid,
    /// Optional built-in `code` template (highlight.js inline; `--param lang=`).
    Code,
    /// User-supplied template file under `~/.tinyview/templates/<name>.html`.
    User(PathBuf),
}

/// Data injected into `window.__TINYVIEW__`.
pub struct InjectData<'a> {
    pub input: &'a str,
    pub params: &'a HashMap<String, String>,
    pub title: &'a str,
    pub path: Option<&'a Path>,
}

/// Errors that can occur during template rendering.
#[derive(Debug)]
pub enum RenderError {
    /// Failed to read a user-supplied template file.
    UserTemplateRead(std::io::Error),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::UserTemplateRead(e) => {
                write!(f, "failed to read user template: {e}")
            }
        }
    }
}

impl std::error::Error for RenderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RenderError::UserTemplateRead(e) => Some(e),
        }
    }
}

/// Resolve which template to use.
///
/// Priority (PRD §13):
/// 1. `explicit` (`--template` / `-t`)
/// 2. extension mapping (from config `[extension]`)
/// 3. `default_template` (from config root)
/// 4. fallback: `raw`
///
/// The result is normalized to a [`TemplateRef`]. Names not matching a
/// built-in (`raw` / `text` / `minimal`) are interpreted as a user template
/// stored at `~/.tinyview/templates/<name>.html`. The exact root resolution
/// (XDG handling etc.) is left to the caller via the returned `User` path;
/// this function builds a relative-looking `<name>.html` path so the caller
/// can join it onto the config root.
pub fn resolve(
    explicit: Option<&str>,
    input_path: Option<&Path>,
    extension_map: Option<&HashMap<String, String>>,
    default_template: Option<&str>,
) -> TemplateRef {
    // 1. explicit
    if let Some(name) = explicit {
        return name_to_ref(name);
    }

    // 2. extension mapping
    if let (Some(path), Some(map)) = (input_path, extension_map) {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            // Case-insensitive match on extension to be friendly with `.MD`
            // etc. The map itself is consulted as-is first, then lowercased.
            if let Some(name) = map.get(ext).or_else(|| map.get(&ext.to_ascii_lowercase())) {
                return name_to_ref(name);
            }
        }
    }

    // 3. default_template
    if let Some(name) = default_template {
        return name_to_ref(name);
    }

    // 4. fallback
    TemplateRef::Raw
}

/// Map a template name string to a [`TemplateRef`].
///
/// Unknown names map to `TemplateRef::User(<name>.html)`. The caller is
/// expected to join this onto the templates root directory.
fn name_to_ref(name: &str) -> TemplateRef {
    match name {
        "raw" => TemplateRef::Raw,
        "text" => TemplateRef::Text,
        "minimal" => TemplateRef::Minimal,
        "markdown" => TemplateRef::Markdown,
        "mermaid" => TemplateRef::Mermaid,
        "code" => TemplateRef::Code,
        other => {
            // Construct a relative file name `<name>.html`. Caller resolves
            // against the config root (e.g. `~/.tinyview/templates/`).
            let mut p = PathBuf::from(other);
            p.set_extension("html");
            TemplateRef::User(p)
        }
    }
}

/// Render a template by substituting the marker with a JSON literal carrying
/// the inject payload.
///
/// - `TemplateRef::Raw` panics: the caller is contractually required to skip
///   `render` for raw (PRD §13.1). Calling here is a programmer error.
/// - Built-in templates use the embedded HTML (no filesystem touch).
/// - User templates are read from disk.
///
/// If the marker is missing the original HTML is returned unchanged and a
/// warning is emitted on stderr (PRD §14.1).
pub fn render(tpl: &TemplateRef, data: &InjectData<'_>) -> Result<String, RenderError> {
    let source: std::borrow::Cow<'_, str> = match tpl {
        TemplateRef::Raw => {
            // PRD §13.1: raw must bypass rendering entirely. Calling here
            // indicates a logic error in the caller.
            panic!("template::render must not be called for TemplateRef::Raw");
        }
        TemplateRef::Text => std::borrow::Cow::Borrowed(TEXT_HTML),
        TemplateRef::Minimal => std::borrow::Cow::Borrowed(MINIMAL_HTML),
        // Replace `LIB_HLJS_CSS` before `LIB_HLJS`: the two placeholders do not
        // overlap (`..._HLJS__*/` vs `..._HLJS_CSS__*/`), but doing the longer
        // one first keeps the ordering robust against future placeholder renames.
        TemplateRef::Markdown => std::borrow::Cow::Owned(
            MARKDOWN_HTML
                .replace(LIB_HLJS_CSS, HLJS_CSS)
                .replace(LIB_MARKED, MARKED_JS)
                .replace(LIB_HLJS, HLJS_JS),
        ),
        TemplateRef::Code => std::borrow::Cow::Owned(
            CODE_HTML
                .replace(LIB_HLJS_CSS, HLJS_CSS)
                .replace(LIB_HLJS, HLJS_JS),
        ),
        TemplateRef::Mermaid => {
            std::borrow::Cow::Owned(MERMAID_HTML.replace(LIB_MERMAID, MERMAID_JS))
        }
        TemplateRef::User(path) => match std::fs::read_to_string(path) {
            Ok(s) => std::borrow::Cow::Owned(s),
            Err(e) => return Err(RenderError::UserTemplateRead(e)),
        },
    };

    let literal = build_literal(data);
    Ok(substitute_marker(&source, &literal))
}

/// Build the JSON literal that replaces the marker.
fn build_literal(data: &InjectData<'_>) -> String {
    let payload = serde_json::json!({
        "input": data.input,
        "params": data.params,
        "title": data.title,
        "path": data.path.and_then(|p| p.to_str()),
    });
    payload.to_string()
}

/// Substitute the marker once. If the marker is missing emit a warning and
/// return the original HTML unchanged.
fn substitute_marker(source: &str, literal: &str) -> String {
    if let Some(idx) = source.find(MARKER) {
        let mut out = String::with_capacity(source.len() + literal.len());
        out.push_str(&source[..idx]);
        out.push_str(literal);
        out.push_str(&source[idx + MARKER.len()..]);
        out
    } else {
        eprintln!(
            "tinyview: warning: template has no `{}` marker; injection skipped",
            MARKER
        );
        source.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_map() -> HashMap<String, String> {
        HashMap::new()
    }

    #[test]
    fn resolve_explicit_wins_over_everything() {
        let mut ext: HashMap<String, String> = HashMap::new();
        ext.insert("md".to_string(), "markdown".to_string());

        let got = resolve(
            Some("text"),
            Some(Path::new("README.md")),
            Some(&ext),
            Some("minimal"),
        );
        assert_eq!(got, TemplateRef::Text);
    }

    #[test]
    fn resolve_extension_used_when_no_explicit() {
        let mut ext: HashMap<String, String> = HashMap::new();
        ext.insert("md".to_string(), "minimal".to_string());

        let got = resolve(None, Some(Path::new("notes.md")), Some(&ext), Some("text"));
        assert_eq!(got, TemplateRef::Minimal);
    }

    #[test]
    fn resolve_default_used_when_no_extension_match() {
        let ext: HashMap<String, String> = HashMap::new();
        let got = resolve(None, Some(Path::new("a.unknown")), Some(&ext), Some("text"));
        assert_eq!(got, TemplateRef::Text);
    }

    #[test]
    fn resolve_falls_back_to_raw() {
        let got = resolve(None, None, None, None);
        assert_eq!(got, TemplateRef::Raw);
    }

    #[test]
    fn resolve_unknown_name_becomes_user_template() {
        let got = resolve(Some("custom-layout"), None, None, None);
        assert_eq!(got, TemplateRef::User(PathBuf::from("custom-layout.html")));
    }

    #[test]
    fn render_substitutes_marker_once() {
        let params = empty_map();
        let data = InjectData {
            input: "<h1>Hi</h1>",
            params: &params,
            title: "tinyview",
            path: None,
        };
        let out = render(&TemplateRef::Text, &data).expect("render ok");

        // The marker should be gone, replaced exactly once.
        assert!(!out.contains(MARKER), "marker should have been substituted");
        // Should contain a JSON object with our input.
        assert!(
            out.contains(r#""input":"<h1>Hi</h1>""#),
            "input not present in JSON literal: {out}"
        );
        // Title is propagated.
        assert!(out.contains(r#""title":"tinyview""#));
        // Path null when no path.
        assert!(out.contains(r#""path":null"#));
    }

    #[test]
    fn render_minimal_includes_params() {
        let mut params = HashMap::new();
        params.insert("theme".to_string(), "github".to_string());
        let data = InjectData {
            input: "hello",
            params: &params,
            title: "t",
            path: Some(Path::new("/tmp/x.md")),
        };
        let out = render(&TemplateRef::Minimal, &data).expect("render ok");
        assert!(out.contains(r#""theme":"github""#));
        assert!(out.contains(r#""path":"/tmp/x.md""#));
        assert!(!out.contains(MARKER));
    }

    #[test]
    fn render_marker_missing_returns_source_unchanged() {
        // Build a fake user template with no marker, write to a temp file,
        // then render. We use env::temp_dir to avoid extra dev-deps.
        let dir = std::env::temp_dir();
        let path = dir.join("tinyview_test_no_marker.html");
        let html = "<html><body>no marker here</body></html>";
        std::fs::write(&path, html).expect("write tmp template");

        let params = empty_map();
        let data = InjectData {
            input: "x",
            params: &params,
            title: "t",
            path: None,
        };
        let out = render(&TemplateRef::User(path.clone()), &data).expect("render ok");
        assert_eq!(out, html);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn resolve_optional_builtin_names() {
        assert_eq!(
            resolve(Some("markdown"), None, None, None),
            TemplateRef::Markdown
        );
        assert_eq!(
            resolve(Some("mermaid"), None, None, None),
            TemplateRef::Mermaid
        );
        assert_eq!(resolve(Some("code"), None, None, None), TemplateRef::Code);
    }

    #[test]
    fn render_markdown_inlines_libs_and_marker() {
        let params = empty_map();
        let data = InjectData {
            input: "Hello markdown",
            params: &params,
            title: "tinyview",
            path: Some(Path::new("README.md")),
        };
        let out = render(&TemplateRef::Markdown, &data).expect("render ok");
        // Data marker substituted, lib placeholders gone.
        assert!(!out.contains(MARKER));
        assert!(!out.contains(LIB_MARKED));
        assert!(!out.contains(LIB_HLJS));
        assert!(!out.contains(LIB_HLJS_CSS));
        // Vendored libraries actually inlined (self-contained, no external refs).
        assert!(out.contains("marked"), "marked.js not inlined");
        assert!(out.contains("hljs"), "highlight.js not inlined");
        assert!(out.contains(r#""input":"Hello markdown""#));
        // No external resource references (No Server contract).
        assert!(!out.contains("<script src="));
        assert!(!out.contains("<link "));
    }

    #[test]
    fn render_code_inlines_hljs() {
        let mut params = HashMap::new();
        params.insert("lang".to_string(), "rust".to_string());
        let data = InjectData {
            input: "fn main() {}",
            params: &params,
            title: "t",
            path: None,
        };
        let out = render(&TemplateRef::Code, &data).expect("render ok");
        assert!(!out.contains(MARKER));
        assert!(!out.contains(LIB_HLJS));
        assert!(!out.contains(LIB_HLJS_CSS));
        assert!(out.contains("hljs"), "highlight.js not inlined");
        assert!(out.contains(r#""lang":"rust""#));
    }

    #[test]
    fn render_mermaid_inlines_lib() {
        let params = empty_map();
        let data = InjectData {
            input: "graph TD; A-->B",
            params: &params,
            title: "t",
            path: None,
        };
        let out = render(&TemplateRef::Mermaid, &data).expect("render ok");
        assert!(!out.contains(MARKER));
        assert!(!out.contains(LIB_MERMAID));
        assert!(out.contains("mermaid"), "mermaid.js not inlined");
        assert!(out.contains(r#""input":"graph TD; A-->B""#));
        assert!(!out.contains("<script src="));
    }

    #[test]
    #[should_panic(expected = "must not be called for TemplateRef::Raw")]
    fn render_panics_on_raw() {
        let params = empty_map();
        let data = InjectData {
            input: "x",
            params: &params,
            title: "t",
            path: None,
        };
        let _ = render(&TemplateRef::Raw, &data);
    }
}
