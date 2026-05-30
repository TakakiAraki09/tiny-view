use std::cell::OnceCell;
use std::collections::HashMap;
use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;

mod config;
mod detach;
mod template;
mod watch;
mod webview;

use template::TemplateRef;
use watch::UserEvent;
use webview::{BuildOptions, Permissions};

#[derive(Parser)]
#[command(name = "tinyview", version, about = "Ephemeral CLI WebView runtime")]
struct Cli {
    /// Path to an HTML file
    source: Option<PathBuf>,

    /// Inline HTML string
    #[arg(long)]
    html: Option<String>,

    /// Template name (raw / text / minimal / <user>)
    #[arg(short = 't', long)]
    template: Option<String>,

    /// Template parameter (repeatable). Format: key=value
    #[arg(long, value_parser = parse_param)]
    param: Vec<(String, String)>,

    /// Window width
    #[arg(long)]
    width: Option<u32>,

    /// Window height
    #[arg(long)]
    height: Option<u32>,

    /// Frameless window — remove title bar and window decorations (PRD §9.8).
    #[arg(long)]
    frameless: bool,

    /// Transparent window background (PRD §9.9). Combine with
    /// `rgba(_,_,_,<1)` in your HTML/CSS to see through to the desktop.
    /// On macOS this uses a private WKWebView API via wry's `transparent` feature.
    #[arg(long)]
    transparent: bool,

    /// Stay in foreground (skip detach). Useful for --watch / CI / debug.
    #[arg(long)]
    foreground: bool,

    /// Watch the source file for changes and reload the WebView (file input only).
    /// Implies `--foreground`.
    #[arg(long)]
    watch: bool,

    /// Allow outbound fetch / XHR / WebSocket (relaxes CSP connect-src)
    #[arg(long)]
    allow_fetch: bool,

    /// Allow clipboard API (no-op on macOS native shortcuts)
    #[arg(long)]
    allow_clipboard: bool,

    /// Persist WebView storage across runs (disables incognito)
    #[arg(long)]
    allow_storage: bool,
}

fn parse_param(s: &str) -> Result<(String, String), String> {
    let (k, v) = s
        .split_once('=')
        .ok_or_else(|| format!("expected key=value, got `{s}`"))?;
    Ok((k.to_string(), v.to_string()))
}

struct Input {
    content: String,
    path: Option<PathBuf>,
}

fn read_input(cli: &Cli) -> io::Result<Input> {
    let stdin = io::stdin();
    // Prefer stdin only when (a) it's not a terminal AND (b) it actually has
    // data ready. This avoids accidentally consuming an empty `/dev/null` stdin
    // (e.g. background jobs, cron, sandboxed shells) when the user provided an
    // explicit file or --html argument.
    if !stdin.is_terminal() && stdin_has_data() {
        let mut buf = String::new();
        stdin.lock().read_to_string(&mut buf)?;
        return Ok(Input {
            content: buf,
            path: None,
        });
    }
    if let Some(path) = &cli.source {
        let content = std::fs::read_to_string(path)?;
        return Ok(Input {
            content,
            path: Some(path.clone()),
        });
    }
    if let Some(html) = &cli.html {
        return Ok(Input {
            content: html.clone(),
            path: None,
        });
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "no input: pipe stdin, pass a file, or use --html",
    ))
}

/// FIONREAD ioctl: returns true if stdin has at least one byte ready to read.
/// Used to distinguish `cat file | tinyview` (data ready) from `tinyview &`
/// (stdin redirected to /dev/null with no data).
#[cfg(unix)]
fn stdin_has_data() -> bool {
    use std::os::fd::AsRawFd;
    let mut bytes: libc::c_int = 0;
    // SAFETY: FIONREAD on a valid fd is well-defined; we pass a writable int.
    let r = unsafe { libc::ioctl(io::stdin().as_raw_fd(), libc::FIONREAD, &mut bytes) };
    r == 0 && bytes > 0
}

#[cfg(not(unix))]
fn stdin_has_data() -> bool {
    true
}

/// PRD §13.1: raw fast path skips config load entirely.
fn is_raw_fast_path(cli: &Cli, input: &Input) -> bool {
    cli.template.is_none() && cli.param.is_empty() && input.path.is_none()
}

/// Resolve `User(<name>.html)` to an absolute path under the config root's
/// `templates/` dir. Shares the same fallback chain as `config.toml`
/// (`$XDG_CONFIG_HOME` > `~/.config/tinyview` > `~/.tinyview`) via
/// [`config::config_root`] (PRD §11.1).
fn resolve_user_template_path(tpl: TemplateRef) -> TemplateRef {
    match tpl {
        TemplateRef::User(rel) if rel.is_relative() => {
            let root = config::config_root()
                .map(|r| r.join("templates"))
                .unwrap_or_else(|| PathBuf::from("."));
            TemplateRef::User(root.join(rel))
        }
        other => other,
    }
}

/// Merge config `[templates.X.params]` with CLI `--param` (CLI wins).
fn merge_params(
    tpl: &TemplateRef,
    cfg: Option<&config::Config>,
    cli_params: &[(String, String)],
) -> HashMap<String, String> {
    let mut out: HashMap<String, String> = HashMap::new();

    let name: Option<&str> = match tpl {
        TemplateRef::Raw => Some("raw"),
        TemplateRef::Text => Some("text"),
        TemplateRef::Minimal => Some("minimal"),
        TemplateRef::Markdown => Some("markdown"),
        TemplateRef::Mermaid => Some("mermaid"),
        TemplateRef::Code => Some("code"),
        TemplateRef::User(p) => p.file_stem().and_then(|s| s.to_str()),
    };

    if let (Some(c), Some(n)) = (cfg, name) {
        if let Some(entry) = c.templates.get(n) {
            for (k, v) in &entry.params {
                out.insert(k.clone(), v.clone());
            }
        }
    }

    for (k, v) in cli_params {
        out.insert(k.clone(), v.clone());
    }

    out
}

/// Window-level configuration bundled together so we can grow this without
/// inflating every internal signature. Width / height stay primitive because
/// they have config-driven defaults; frameless / transparent are pure CLI flags.
struct WindowOpts {
    width: u32,
    height: u32,
    /// PRD §9.8 — strip title bar and OS chrome.
    frameless: bool,
    /// PRD §9.9 — transparent window background. Requires the WebView itself
    /// to be transparent too; both are wired together in [`launch_webview`].
    transparent: bool,
}

/// Launch the WebView and run the event loop. Diverges on macOS
/// (`event_loop.run` is `-> !`).
///
/// If `watch_ctx` is `Some`, a notify watcher is spawned that re-renders the
/// source file on change and forwards a `UserEvent::Reload` through the
/// event loop proxy. The watcher guard is held inside this function so it
/// lives exactly as long as the event loop.
fn launch_webview(
    window_opts: WindowOpts,
    html: String,
    perms: Permissions,
    raw_mode: bool,
    watch_ctx: Option<watch::WatchContext>,
) -> ExitCode {
    // Always use `EventLoop<UserEvent>` — even when watch is off — so the
    // event-loop closure type is uniform and we avoid duplicating the run
    // body for two different event types.
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();

    // Transparency is a *two-layer* contract:
    //   1. tao's NSWindow / HWND must be transparent so the OS compositor
    //      doesn't fill the background with the system window color.
    //   2. wry's WKWebView / WebView2 / WebKitGTK must skip drawing its own
    //      opaque background (otherwise it just paints over the transparent
    //      window). See webview::build for the wry side.
    // Frameless windows aren't draggable by default on macOS — that's an
    // intentional tao behavior; users who want a draggable transparent window
    // can use CSS `-webkit-app-region: drag` in their HTML.
    let window = match WindowBuilder::new()
        .with_title("tinyview")
        .with_inner_size(LogicalSize::new(
            window_opts.width as f64,
            window_opts.height as f64,
        ))
        .with_decorations(!window_opts.frameless)
        .with_transparent(window_opts.transparent)
        .build(&event_loop)
    {
        Ok(w) => w,
        Err(e) => {
            eprintln!("tinyview: failed to create window: {e}");
            return ExitCode::from(1);
        }
    };

    let webview = match webview::build(
        &window,
        BuildOptions {
            html: &html,
            perms,
            raw_mode,
            transparent: window_opts.transparent,
        },
    ) {
        Ok(wv) => wv,
        Err(e) => {
            eprintln!("tinyview: failed to create webview: {e}");
            return ExitCode::from(1);
        }
    };

    // Spawn the notify watcher (if requested). The returned guard must
    // outlive `event_loop.run` — moved into the closure below.
    let _watcher_guard = match watch_ctx {
        Some(ctx) => match watch::spawn_watcher(ctx, event_loop.create_proxy()) {
            Ok(g) => Some(g),
            Err(e) => {
                // Non-fatal: log and continue without watch. The user still
                // gets a working WebView; only auto-reload is lost.
                eprintln!("tinyview: warn: failed to start file watcher: {e}");
                None
            }
        },
        None => None,
    };

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            Event::UserEvent(UserEvent::Reload(new_html)) => {
                // Re-apply the same CSP `<meta>` injection as the initial
                // render so reloads don't drop security headers (PRD §19).
                let prepared = webview::prepare_html(&new_html, &perms, raw_mode);
                if let Err(e) = webview.load_html(&prepared) {
                    eprintln!("tinyview: warn: load_html failed: {e}");
                }
            }
            _ => {}
        }
    });
}

/// Detached child path: skip input / template logic, read pre-composed HTML
/// from stdin pipe and launch the WebView directly.
fn run_as_child(cli: Cli) -> ExitCode {
    let mut html = String::new();
    if io::stdin().lock().read_to_string(&mut html).is_err() {
        return ExitCode::from(1);
    }

    let window_opts = WindowOpts {
        width: cli.width.unwrap_or(1000),
        height: cli.height.unwrap_or(760),
        frameless: cli.frameless,
        transparent: cli.transparent,
    };
    let raw_mode = detach::detached_raw_mode();
    let perms = Permissions {
        allow_fetch: cli.allow_fetch,
        allow_clipboard: cli.allow_clipboard,
        allow_storage: cli.allow_storage,
    };

    launch_webview(window_opts, html, perms, raw_mode, None)
}

/// Parent / foreground path: full pipeline including template resolution.
fn run(cli: Cli) -> ExitCode {
    let input = match read_input(&cli) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("tinyview: {e}");
            return ExitCode::from(1);
        }
    };

    // `--watch` is only meaningful for file input (PRD §9.10). Stdin is a
    // one-shot stream and `--html` is an inline literal — neither has a path
    // we can watch. Reject early with a clear error rather than silently
    // ignoring the flag.
    if cli.watch && input.path.is_none() {
        eprintln!("tinyview: --watch requires a file path (stdin / --html not supported)");
        return ExitCode::from(2);
    }

    let cfg_cache: OnceCell<Option<config::Config>> = OnceCell::new();
    let cfg = if is_raw_fast_path(&cli, &input) {
        None
    } else {
        config::load_if_needed(&cfg_cache)
    };

    let tpl = template::resolve(
        cli.template.as_deref(),
        input.path.as_deref(),
        cfg.map(|c| &c.extension),
        cfg.and_then(|c| c.default_template.as_deref()),
    );
    let tpl = resolve_user_template_path(tpl);

    let merged_params = merge_params(&tpl, cfg, &cli.param);
    let raw_mode = matches!(tpl, TemplateRef::Raw);

    let html = if raw_mode {
        input.content
    } else {
        let data = template::InjectData {
            input: &input.content,
            params: &merged_params,
            title: "tinyview",
            path: input.path.as_deref(),
        };
        match template::render(&tpl, &data) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("tinyview: {e}");
                return ExitCode::from(1);
            }
        }
    };

    let width = cli
        .width
        .or(cfg.and_then(|c| c.window_width))
        .unwrap_or(1000);
    let height = cli
        .height
        .or(cfg.and_then(|c| c.window_height))
        .unwrap_or(760);

    let perms = Permissions {
        allow_fetch: cli.allow_fetch,
        allow_clipboard: cli.allow_clipboard,
        allow_storage: cli.allow_storage,
    };

    // `--watch` implies foreground: detaching would require a parent→child
    // protocol to ferry source/template/params, which we explicitly avoid
    // (see PRD §9.10 — watch is for interactive use, foreground is fine).
    let foreground = cli.foreground || cli.watch;

    if foreground {
        let watch_ctx = if cli.watch {
            // SAFETY of unwrap: validated above (`cli.watch && input.path.is_none()`
            // returns early), so `input.path` is `Some` here.
            let source = input.path.clone().expect("watch validated above");
            Some(watch::WatchContext {
                source,
                template: tpl.clone(),
                params: merged_params.clone(),
                raw_mode,
            })
        } else {
            None
        };
        let window_opts = WindowOpts {
            width,
            height,
            frameless: cli.frameless,
            transparent: cli.transparent,
        };
        return launch_webview(window_opts, html, perms, raw_mode, watch_ctx);
    }

    // Detach: spawn ourselves as a detached child via Command::spawn,
    // write composed HTML to its stdin, then parent exits.
    let opts = detach::SpawnOpts {
        html: &html,
        width,
        height,
        raw_mode,
        allow_fetch: cli.allow_fetch,
        allow_clipboard: cli.allow_clipboard,
        allow_storage: cli.allow_storage,
        frameless: cli.frameless,
        transparent: cli.transparent,
    };

    match detach::spawn(&opts) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("tinyview: detach failed: {e}");
            ExitCode::from(1)
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    if detach::is_detached_child() {
        return run_as_child(cli);
    }

    run(cli)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn cli_defaults_frameless_and_transparent_to_false() {
        let cli = Cli::try_parse_from(["tinyview", "file.html"]).expect("parse");
        assert!(!cli.frameless);
        assert!(!cli.transparent);
    }

    #[test]
    fn cli_parses_frameless_flag() {
        let cli = Cli::try_parse_from(["tinyview", "file.html", "--frameless"]).expect("parse");
        assert!(cli.frameless);
        assert!(!cli.transparent);
    }

    #[test]
    fn cli_parses_transparent_flag() {
        let cli = Cli::try_parse_from(["tinyview", "file.html", "--transparent"]).expect("parse");
        assert!(cli.transparent);
        assert!(!cli.frameless);
    }

    #[test]
    fn cli_parses_frameless_and_transparent_together() {
        let cli = Cli::try_parse_from(["tinyview", "file.html", "--frameless", "--transparent"])
            .expect("parse");
        assert!(cli.frameless);
        assert!(cli.transparent);
    }

    #[test]
    fn cli_frameless_and_transparent_coexist_with_existing_flags() {
        // Regression guard: make sure adding the new flags didn't shadow or
        // collide with width/height/--allow-fetch/--foreground argument parsing.
        let cli = Cli::try_parse_from([
            "tinyview",
            "file.html",
            "--frameless",
            "--transparent",
            "--width",
            "640",
            "--height",
            "480",
            "--allow-fetch",
            "--foreground",
        ])
        .expect("parse");
        assert!(cli.frameless);
        assert!(cli.transparent);
        assert_eq!(cli.width, Some(640));
        assert_eq!(cli.height, Some(480));
        assert!(cli.allow_fetch);
        assert!(cli.foreground);
    }

    #[test]
    fn merge_params_code_cli_lang_overrides_config() {
        // `[templates.code.params] lang = "python"` in config, overridden by
        // `--param lang=rust` on the CLI (CLI wins). Regression guard for the
        // new optional built-in template name -> config key mapping.
        let mut cfg = config::Config::default();
        let mut entry = config::TemplateEntry::default();
        entry
            .params
            .insert("lang".to_string(), "python".to_string());
        entry
            .params
            .insert("theme".to_string(), "github".to_string());
        cfg.templates.insert("code".to_string(), entry);

        let cli_params = vec![("lang".to_string(), "rust".to_string())];
        let merged = merge_params(&TemplateRef::Code, Some(&cfg), &cli_params);

        assert_eq!(merged.get("lang").map(String::as_str), Some("rust"));
        // Config-only params survive when not overridden.
        assert_eq!(merged.get("theme").map(String::as_str), Some("github"));
    }
}
