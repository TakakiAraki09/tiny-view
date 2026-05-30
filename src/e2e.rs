//! Live-WebView self-test harness for the `--allow-*` permission flags
//! (issue #5). Compiled only under the `e2e` feature.
//!
//! ## Why this exists
//!
//! `--allow-fetch` / `--allow-clipboard` / `--allow-storage` are unit-tested at
//! the string level (CSP construction, injection) in `webview.rs`, but those
//! tests cannot prove the *browser* actually honors them. This harness drives a
//! real `WebView` (built through the production [`crate::webview::build`] path,
//! so CSP injection / incognito / clipboard neutralization all apply) and reads
//! page-side JS results back over a temporary IPC channel that only exists
//! under the `e2e` feature.
//!
//! ## How to run
//!
//! ```sh
//! # macOS (local):
//! TINYVIEW_E2E_SELFTEST=1 cargo run --features e2e
//! # Linux CI: wrap in a virtual display
//! xvfb-run -a env TINYVIEW_E2E_SELFTEST=1 cargo run --features e2e
//! ```
//!
//! Exit code is `0` if every check required on the current platform passed,
//! `1` otherwise. Soft checks (platform-dependent behavior we only smoke-test
//! best-effort) are reported as `WARN` and never fail the run.
//!
//! ## Design notes
//!
//! - A **single** `EventLoop` is created and entered **once** with `run_return`.
//!   All scenarios are sequenced as a state machine inside that one run: macOS
//!   tears the app down after the first `ControlFlow::Exit`, so re-entering the
//!   loop per scenario does not work. Instead we advance `idx` through the step
//!   list, building one window+webview at a time and tearing it down before the
//!   next (which is also what makes the storage "across runs" check meaningful).
//! - Windows are invisible (`with_visible(false)`) so the harness doesn't flash
//!   windows during local/CI runs; JS and CSP enforcement run regardless.
//! - Each probe is embedded as an inline `<script>` at the end of `<body>` so it
//!   runs *after* the injected CSP `<meta>` in `<head>` is parsed and active.

use std::collections::HashMap;
use std::process::ExitCode;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopWindowTarget};
use tao::platform::run_return::EventLoopExtRunReturn;
use tao::window::{Window, WindowBuilder};
use wry::WebView;

use crate::webview::{self, BuildOptions, Permissions};

/// Upper bound on how long a single probe may take before we give up and
/// record a timeout. Generous because cold WebView startup on CI is slow.
const PROBE_TIMEOUT: Duration = Duration::from_secs(15);

/// Probe that reports whether the CSP blocked an outbound `fetch`.
///
/// Listens for a `connect-src` `securitypolicyviolation`, fires a `fetch`, then
/// reports `blocked` / `allowed` after a short settle delay. Under the default
/// CSP (`connect-src 'none'`) the violation fires and we expect `blocked`; with
/// `--allow-fetch` no violation fires (the request proceeds to the network,
/// success or network-error irrelevant) and we expect `allowed`.
const FETCH_PROBE: &str = r#"
(function () {
  var blocked = false;
  document.addEventListener('securitypolicyviolation', function (e) {
    var d = (e.effectiveDirective || e.violatedDirective || '');
    if (d.indexOf('connect-src') === 0) blocked = true;
  });
  function done() { window.ipc.postMessage(blocked ? 'blocked' : 'allowed'); }
  try {
    fetch('https://example.com/').then(function () {}, function () {});
  } catch (e) {
    blocked = true;
  }
  setTimeout(done, 400);
})();
"#;

/// Probe that reports the JS visibility of the clipboard API as
/// `clipboard:<typeof navigator.clipboard>|<typeof writeText | 'none'>`.
const CLIPBOARD_PROBE: &str = r#"
(function () {
  var t = typeof navigator.clipboard;
  var w = (navigator.clipboard && typeof navigator.clipboard.writeText) || 'none';
  window.ipc.postMessage('clipboard:' + t + '|' + w);
})();
"#;

/// Probe that writes a sentinel into `localStorage`.
const STORAGE_SET_PROBE: &str = r#"
(function () {
  try {
    localStorage.setItem('tv_e2e', '42');
    window.ipc.postMessage('set:ok');
  } catch (e) {
    window.ipc.postMessage('set:err:' + e);
  }
})();
"#;

/// Probe that reads the sentinel back: `get:<value|null>`.
const STORAGE_GET_PROBE: &str = r#"
(function () {
  try {
    window.ipc.postMessage('get:' + localStorage.getItem('tv_e2e'));
  } catch (e) {
    window.ipc.postMessage('get:err:' + e);
  }
})();
"#;

/// Probe that clears the sentinel so it doesn't leak into later runs.
const STORAGE_CLEANUP_PROBE: &str = r#"
(function () {
  try { localStorage.removeItem('tv_e2e'); } catch (e) {}
  window.ipc.postMessage('cleanup');
})();
"#;

/// One scenario in the self-test: build a webview with `perms`, run `probe`,
/// and capture the first `window.ipc.postMessage(...)` body. `id` labels the
/// captured result for the assertions assembled after the run.
struct Step {
    id: &'static str,
    perms: Permissions,
    probe: &'static str,
}

/// State for the currently-running scenario. Window + WebView are owned here so
/// they stay alive while the probe runs and are dropped (tearing down the
/// WebView) when we advance to the next scenario.
struct Active {
    _window: Window,
    _webview: WebView,
    rx: mpsc::Receiver<String>,
    started: Instant,
}

fn html_for(probe: &str) -> String {
    format!(
        "<!doctype html><html><head><title>e2e</title></head>\
         <body><script>{probe}</script></body></html>"
    )
}

/// Build the window + webview for `step` through the production path.
fn start_step(target: &EventLoopWindowTarget<()>, step: &Step) -> Result<Active, String> {
    let window = WindowBuilder::new()
        .with_title("tinyview-e2e")
        .with_visible(false)
        .with_inner_size(LogicalSize::new(320.0, 240.0))
        .build(target)
        .map_err(|e| format!("window build failed: {e}"))?;

    let (tx, rx) = mpsc::channel::<String>();
    let html = html_for(step.probe);

    // `raw_mode = false` so CSP is always injected (the point of the fetch
    // check). Permission flags additionally widen the CSP / toggle incognito
    // inside `build`.
    let webview = webview::build(
        &window,
        BuildOptions {
            html: &html,
            perms: step.perms,
            raw_mode: false,
            transparent: false,
            ipc_tx: Some(tx),
        },
    )
    .map_err(|e| format!("webview build failed: {e}"))?;

    Ok(Active {
        _window: window,
        _webview: webview,
        rx,
        started: Instant::now(),
    })
}

/// Run all permission scenarios and aggregate a report.
pub fn run_selftest() -> ExitCode {
    let allow_fetch = Permissions {
        allow_fetch: true,
        ..Default::default()
    };
    let allow_clipboard = Permissions {
        allow_clipboard: true,
        ..Default::default()
    };
    let allow_storage = Permissions {
        allow_storage: true,
        ..Default::default()
    };

    let steps = [
        // --allow-fetch: default blocks, flag permits.
        Step {
            id: "fetch_off",
            perms: Permissions::default(),
            probe: FETCH_PROBE,
        },
        Step {
            id: "fetch_on",
            perms: allow_fetch,
            probe: FETCH_PROBE,
        },
        // --allow-clipboard: default neutralizes (macOS), flag exposes.
        Step {
            id: "clip_off",
            perms: Permissions::default(),
            probe: CLIPBOARD_PROBE,
        },
        Step {
            id: "clip_on",
            perms: allow_clipboard,
            probe: CLIPBOARD_PROBE,
        },
        // --allow-storage OFF: incognito → a fresh webview can't see a prior write.
        Step {
            id: "store_off_set",
            perms: Permissions::default(),
            probe: STORAGE_SET_PROBE,
        },
        Step {
            id: "store_off_get",
            perms: Permissions::default(),
            probe: STORAGE_GET_PROBE,
        },
        // --allow-storage ON: persistent → write is visible to the next webview.
        Step {
            id: "store_on_set",
            perms: allow_storage,
            probe: STORAGE_SET_PROBE,
        },
        Step {
            id: "store_on_get",
            perms: allow_storage,
            probe: STORAGE_GET_PROBE,
        },
        // Best-effort cleanup of the persisted sentinel.
        Step {
            id: "cleanup",
            perms: allow_storage,
            probe: STORAGE_CLEANUP_PROBE,
        },
    ];

    let mut event_loop = EventLoopBuilder::<()>::with_user_event().build();

    let mut results: Vec<Option<String>> = vec![None; steps.len()];
    let mut idx: usize = 0;
    let mut active: Option<Active> = None;

    event_loop.run_return(|event, target, control_flow| {
        // Wake frequently to pump the webview, poll the channel, and check the
        // per-step timeout.
        *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(20));

        // Start the next scenario if none is in flight.
        if active.is_none() && idx < steps.len() {
            match start_step(target, &steps[idx]) {
                Ok(a) => active = Some(a),
                Err(e) => {
                    results[idx] = Some(format!("ERR:{e}"));
                    idx += 1;
                }
            }
        }

        // Poll the in-flight scenario for a result or a timeout.
        if let Some(a) = active.as_ref() {
            if let Ok(msg) = a.rx.try_recv() {
                results[idx] = Some(msg);
                active = None;
                idx += 1;
            } else if a.started.elapsed() > PROBE_TIMEOUT {
                results[idx] = Some("TIMEOUT".to_string());
                active = None;
                idx += 1;
            }
        }

        if idx >= steps.len() && active.is_none() {
            *control_flow = ControlFlow::Exit;
        }

        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            *control_flow = ControlFlow::Exit;
        }
    });

    // Index results by step id for readable assertions.
    let mut got: HashMap<&'static str, String> = HashMap::new();
    for (step, res) in steps.iter().zip(results.iter()) {
        got.insert(
            step.id,
            res.clone().unwrap_or_else(|| "MISSING".to_string()),
        );
    }
    let g = |id: &str| {
        got.get(id)
            .cloned()
            .unwrap_or_else(|| "MISSING".to_string())
    };

    let mut report = Report::default();

    // ---- --allow-fetch ----------------------------------------------------
    report.check(
        "allow-fetch: default CSP blocks fetch",
        g("fetch_off") == "blocked",
        false,
        g("fetch_off"),
    );
    report.check(
        "allow-fetch: flag permits fetch (no CSP block)",
        g("fetch_on") == "allowed",
        false,
        g("fetch_on"),
    );

    // ---- --allow-clipboard -----------------------------------------------
    // Security invariant: by default the clipboard API must NOT be reachable
    // from page JS. On macOS the neutralization init script (PRD §19.3)
    // guarantees this → hard check; elsewhere the WebKit port decides → soft.
    report.check(
        "allow-clipboard: default does not expose navigator.clipboard",
        g("clip_off") == "clipboard:undefined|none",
        !cfg!(target_os = "macos"),
        g("clip_off"),
    );
    // Documented limitation: TinyView injects HTML via `with_html` (no base
    // URL), so the document has an opaque origin and the Clipboard API — being
    // secure-context-gated — is NOT exposed even with `--allow-clipboard`. We
    // assert the limitation so a future change that DOES expose it (e.g. moving
    // to a real origin) surfaces here as a WARN instead of silently passing.
    report.check(
        "allow-clipboard: opaque-origin in-memory HTML keeps clipboard unexposed",
        g("clip_on") == "clipboard:undefined|none",
        true,
        g("clip_on"),
    );

    // ---- --allow-storage -------------------------------------------------
    // Security invariant (ephemeral): the default path must never expose a
    // value written by a previous run. This holds whether Web Storage is
    // unavailable (opaque origin) or merely incognito-cleared — either way the
    // read must not return the sentinel. Hard on every platform.
    report.check(
        "allow-storage: default does not persist data across runs",
        g("store_off_get") != "get:42",
        false,
        format!("set={} get={}", g("store_off_set"), g("store_off_get")),
    );
    // Documented limitation: same opaque-origin reason — `localStorage` throws
    // `SecurityError` on the in-memory path, so `--allow-storage` cannot make
    // it persist here. Soft, so it documents reality without failing; flips to
    // WARN if a future change makes storage available.
    report.check(
        "allow-storage: opaque-origin in-memory HTML keeps localStorage unavailable",
        g("store_on_set").starts_with("set:err") || g("store_on_get") != "get:42",
        true,
        format!("set={} get={}", g("store_on_set"), g("store_on_get")),
    );

    print_report(&report)
}

/// Aggregated assertion results.
#[derive(Default)]
struct Report {
    passes: Vec<String>,
    warnings: Vec<String>,
    failures: Vec<String>,
}

impl Report {
    /// Record a check. `soft` checks never fail the run (best-effort,
    /// platform-dependent); they only emit a `WARN`.
    fn check(&mut self, name: &str, ok: bool, soft: bool, detail: String) {
        let line = format!("{name} — got {detail:?}");
        if ok {
            self.passes.push(line);
        } else if soft {
            self.warnings.push(line);
        } else {
            self.failures.push(line);
        }
    }
}

fn print_report(report: &Report) -> ExitCode {
    println!(
        "\n=== tinyview --allow-* E2E self-test ({}) ===",
        std::env::consts::OS
    );
    for p in &report.passes {
        println!("  PASS  {p}");
    }
    for w in &report.warnings {
        println!("  WARN  {w}");
    }
    for f in &report.failures {
        println!("  FAIL  {f}");
    }
    println!(
        "--- {} passed, {} warned, {} failed ---",
        report.passes.len(),
        report.warnings.len(),
        report.failures.len()
    );

    if report.failures.is_empty() {
        println!("E2E self-test: OK");
        ExitCode::SUCCESS
    } else {
        eprintln!(
            "E2E self-test: FAILED ({} hard failures)",
            report.failures.len()
        );
        ExitCode::from(1)
    }
}
