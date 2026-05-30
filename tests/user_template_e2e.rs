//! End-to-end integration test for user templates under
//! `~/.tinyview/templates/<name>.html` (issue #7).
//!
//! ## What this covers
//!
//! `TemplateRef::User` resolution is unit-tested at the string level in
//! `src/template.rs` (e.g. `resolve_unknown_name_becomes_user_template`), but
//! there was no test exercising the *real path* flow:
//!
//! ```text
//! ~/.tinyview/templates/<name>.html → resolve → read → inject → render
//! ```
//!
//! These tests drive the actual `tinyview` binary with `assert_cmd`, placing a
//! real `<name>.html` on disk under a temporary `HOME`, then asserting the
//! composed HTML.
//!
//! ## Why we assert composed HTML, not a live render
//!
//! `tinyview` is a non-blocking CLI: on the normal path it detaches and hands a
//! WebView the composed HTML, returning control to the shell immediately
//! (CLAUDE.md "Non-blocking CLI"). The actual WebView render is GUI-bound and
//! cannot be observed headlessly in CI. So the end-to-end assertion target is
//! the *single composed HTML string* that runtime hands to the WebView — i.e.
//! the output of template resolve + file read + marker injection.
//!
//! To observe that string deterministically, the binary exposes a test-only
//! hook: setting `TINYVIEW_DUMP_HTML` makes `run()` print the composed HTML to
//! stdout and exit *before* launching/detaching a WebView (see
//! `src/main.rs`). The hook writes to stdout only — no server, no port, no
//! generated preview file, no persistent state — so it does not relax any of
//! TinyView's absolute conditions, and it sits off the raw fast path so it does
//! not affect the startup KPI.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use tempfile::TempDir;

/// The injection marker a template must contain to receive `window.__TINYVIEW__`
/// (PRD §14.1). Kept as a literal here (mirrors `src/template.rs`) so the test
/// is independent of the crate's private const.
const MARKER: &str = "/*__TINYVIEW__*/ null /*__TINYVIEW__*/";

/// Create a temp `HOME` with `.tinyview/templates/<name>.html` containing
/// `body`, and return the guard (kept alive for the dir's lifetime).
fn home_with_template(name: &str, body: &str) -> TempDir {
    let home = tempfile::tempdir().expect("create temp HOME");
    let templates = home.path().join(".tinyview").join("templates");
    fs::create_dir_all(&templates).expect("mkdir templates");
    fs::write(templates.join(format!("{name}.html")), body).expect("write template");
    home
}

/// Build a `tinyview` command pinned to a temp `HOME`, with the dump hook on.
///
/// Stdin is intentionally NOT wired here. Tests that assert on injected
/// input/param values feed HTML over stdin (TinyView's #1 input source on every
/// platform — see `read_input` in `src/main.rs`), which is the only
/// cross-platform-robust way to guarantee the HTML reaches the binary: on
/// non-unix targets `stdin_has_data()` is a conservative `true`, so an empty
/// `--write_stdin("")` pipe would be consumed as empty input and `--html` would
/// be ignored. Tests whose assertion is independent of the input value (marker
/// pass-through, missing-file) close stdin and rely on `--html` instead.
fn dump_cmd(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("tinyview").expect("locate tinyview binary");
    cmd.env("HOME", home)
        // Force config_root() onto the legacy `~/.tinyview` chain regardless of
        // the developer's / runner's real XDG env (config.rs::config_root).
        .env_remove("XDG_CONFIG_HOME")
        .env("TINYVIEW_DUMP_HTML", "1");
    cmd
}

/// A minimal but realistic user template carrying the injection marker.
fn template_with_marker() -> String {
    format!(
        "<!doctype html>\n<html><head>\n\
         <script>window.__TINYVIEW__ = {MARKER};</script>\n\
         </head><body><main id=\"out\"></main>\n\
         <script>document.getElementById('out').textContent = \
         window.__TINYVIEW__.input;</script>\n\
         </body></html>",
    )
}

#[test]
fn user_template_resolves_reads_and_injects() {
    // ~/.tinyview/templates/custom.html → resolve → read → inject.
    let home = home_with_template("custom", &template_with_marker());

    // Feed the input over stdin (input source #1 on every platform) so the
    // assertion on the injected value is cross-platform-robust; see `dump_cmd`.
    let assert = dump_cmd(home.path())
        .args(["-t", "custom"])
        .write_stdin("<h1>Hello &amp; world</h1>")
        .assert()
        .success();

    let out = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");

    // The template shell survived (we read the real on-disk file, not a builtin).
    assert!(
        out.contains(r#"<main id="out">"#),
        "template body missing:\n{out}"
    );
    // The marker was substituted exactly once with the JSON literal.
    assert!(!out.contains(MARKER), "marker should be gone:\n{out}");
    // The injected object carries the stdin input verbatim (JSON-encoded).
    assert!(
        out.contains(r#""input":"<h1>Hello &amp; world</h1>""#),
        "input not injected:\n{out}"
    );
    // stdin input has no file path, so `path` is null (PRD §14.2).
    assert!(
        out.contains(r#""path":null"#),
        "path should be null:\n{out}"
    );
    assert!(
        out.contains(r#""title":"tinyview""#),
        "title missing:\n{out}"
    );
}

#[test]
fn user_template_params_are_injected() {
    // `--param k=v` flows into window.__TINYVIEW__.params for user templates.
    let home = home_with_template("themed", &template_with_marker());

    // Input over stdin (#1 everywhere) keeps this cross-platform; the assertion
    // here is on the injected param, not the input value. See `dump_cmd`.
    let assert = dump_cmd(home.path())
        .args(["-t", "themed", "--param", "theme=solarized"])
        .write_stdin("x")
        .assert()
        .success();

    let out = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    assert!(
        out.contains(r#""theme":"solarized""#),
        "param not injected:\n{out}"
    );
}

#[test]
fn user_template_without_marker_passes_through_with_warning() {
    // Edge case: template lacks the marker → injection skipped, original HTML
    // returned unchanged, warning emitted on stderr (PRD §14.1).
    let body = "<!doctype html>\n<html><body>no marker here</body></html>";
    let home = home_with_template("nomarker", body);

    // Input value is irrelevant here: a marker-less template passes through
    // byte-for-byte whatever the input. Close stdin and supply `--html`. (On
    // non-unix `stdin_has_data()` is `true`, so the empty stdin is read as the
    // input, but that has no bearing on the pass-through output we assert.)
    let assert = dump_cmd(home.path())
        .args(["--html", "ignored", "-t", "nomarker"])
        .write_stdin("")
        .assert()
        .success();

    let out = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 stdout");
    let err = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr");

    // Pass-through: composed HTML is byte-identical to the source file.
    assert_eq!(
        out, body,
        "marker-less template must pass through unchanged"
    );
    // Warning surfaced to the user.
    assert!(
        err.contains("no") && err.contains("marker"),
        "expected a marker warning on stderr, got:\n{err}"
    );
}

#[test]
fn missing_user_template_fails_cleanly() {
    // Edge case: referenced template file does not exist → non-zero exit with a
    // clean `tinyview:` error, no panic / no partial stdout.
    let home = tempfile::tempdir().expect("temp HOME");
    fs::create_dir_all(home.path().join(".tinyview").join("templates")).expect("mkdir");

    // Input value is irrelevant: resolution of a non-existent template file
    // fails before any input is injected. Close stdin and supply `--html`.
    let assert = dump_cmd(home.path())
        .args(["--html", "x", "-t", "does-not-exist"])
        .write_stdin("")
        .assert()
        .failure();

    let out = assert.get_output().stdout.clone();
    let err = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8 stderr");

    assert!(out.is_empty(), "no HTML should be emitted on error");
    assert!(
        err.contains("tinyview:") && err.contains("template"),
        "expected a clean template-read error, got:\n{err}"
    );
    // Surfaced as a controlled error, not a panic.
    assert!(!err.contains("panicked"), "should not panic:\n{err}");
}
