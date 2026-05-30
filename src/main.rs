use std::cell::OnceCell;
use std::collections::HashMap;
use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use tao::window::WindowBuilder;

mod config;
mod detach;
mod template;
mod webview;

use template::TemplateRef;
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

    /// Stay in foreground (skip detach). Useful for --watch / CI / debug.
    #[arg(long)]
    foreground: bool,

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
    if !stdin.is_terminal() {
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

/// PRD §13.1: raw fast path skips config load entirely.
fn is_raw_fast_path(cli: &Cli, input: &Input) -> bool {
    cli.template.is_none() && cli.param.is_empty() && input.path.is_none()
}

/// Resolve `User(<name>.html)` to an absolute path under `~/.tinyview/templates/`.
fn resolve_user_template_path(tpl: TemplateRef) -> TemplateRef {
    match tpl {
        TemplateRef::User(rel) if rel.is_relative() => {
            let root = std::env::var_os("HOME")
                .map(|h| PathBuf::from(h).join(".tinyview/templates"))
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

/// Launch the WebView and run the event loop. Diverges on macOS
/// (`event_loop.run` is `-> !`).
fn launch_webview(
    width: u32,
    height: u32,
    html: String,
    perms: Permissions,
    raw_mode: bool,
) -> ExitCode {
    let event_loop = EventLoop::new();
    let window = match WindowBuilder::new()
        .with_title("tinyview")
        .with_inner_size(LogicalSize::new(width as f64, height as f64))
        .build(&event_loop)
    {
        Ok(w) => w,
        Err(e) => {
            eprintln!("tinyview: failed to create window: {e}");
            return ExitCode::from(1);
        }
    };

    let _webview = match webview::build(
        &window,
        BuildOptions {
            html: &html,
            perms,
            raw_mode,
        },
    ) {
        Ok(wv) => wv,
        Err(e) => {
            eprintln!("tinyview: failed to create webview: {e}");
            return ExitCode::from(1);
        }
    };

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            *control_flow = ControlFlow::Exit;
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

    let width = cli.width.unwrap_or(1000);
    let height = cli.height.unwrap_or(760);
    let raw_mode = detach::detached_raw_mode();
    let perms = Permissions {
        allow_fetch: cli.allow_fetch,
        allow_clipboard: cli.allow_clipboard,
        allow_storage: cli.allow_storage,
    };

    launch_webview(width, height, html, perms, raw_mode)
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

    if cli.foreground {
        return launch_webview(width, height, html, perms, raw_mode);
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
