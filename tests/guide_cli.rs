//! Integration test for `tinyview --guide <topic>`.
//!
//! `--guide` is a print-and-exit path: the binary writes a built-in guide
//! (embedded via `include_str!` at compile time) to stdout and returns
//! immediately — no input reading, no config load, no WebView, no detach.
//! That makes it fully observable headlessly, so this test is safe in CI
//! (no GUI required), unlike the WebView render path.
//!
//! Mirrors the `assert_cmd` pattern of `tests/user_template_e2e.rs`.

use assert_cmd::Command;

/// The injection marker documented by the template guide (PRD §14.1). Kept as
/// a literal here (mirrors `src/template.rs`) so the test is independent of
/// the crate's private const.
const MARKER: &str = "/*__TINYVIEW__*/ null /*__TINYVIEW__*/";

fn tinyview() -> Command {
    Command::cargo_bin("tinyview").expect("locate tinyview binary")
}

#[test]
fn guide_template_prints_guide_and_exits_zero() {
    let assert = tinyview().arg("--guide").arg("template").assert().success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf-8 stdout");
    // The guide must document the marker contract and the injected global —
    // the load-bearing pieces a template author (human or AI agent) needs.
    assert!(
        stdout.contains(MARKER),
        "guide output must document the injection marker"
    );
    assert!(
        stdout.contains("window.__TINYVIEW__"),
        "guide output must document the injected global"
    );
}

#[test]
fn guide_rejects_unknown_topic() {
    // clap's ValueEnum handles the error: non-zero exit and a usage error on
    // stderr listing the possible values.
    tinyview().arg("--guide").arg("bogus").assert().failure();
}
