use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

#[derive(Parser)]
#[command(name = "tinyview", version, about = "Ephemeral CLI WebView runtime")]
struct Cli {
    /// Path to an HTML file
    source: Option<PathBuf>,

    /// Inline HTML string
    #[arg(long)]
    html: Option<String>,

    /// Window width
    #[arg(long, default_value_t = 1000)]
    width: u32,

    /// Window height
    #[arg(long, default_value_t = 760)]
    height: u32,
}

fn read_input(cli: &Cli) -> io::Result<String> {
    let stdin = io::stdin();
    if !stdin.is_terminal() {
        let mut buf = String::new();
        stdin.lock().read_to_string(&mut buf)?;
        return Ok(buf);
    }
    if let Some(path) = &cli.source {
        return std::fs::read_to_string(path);
    }
    if let Some(html) = &cli.html {
        return Ok(html.clone());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "no input: pipe stdin, pass a file, or use --html",
    ))
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let html = match read_input(&cli) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("tinyview: {e}");
            return ExitCode::from(1);
        }
    };

    let event_loop = EventLoop::new();
    let window = match WindowBuilder::new()
        .with_title("tinyview")
        .with_inner_size(LogicalSize::new(cli.width as f64, cli.height as f64))
        .build(&event_loop)
    {
        Ok(w) => w,
        Err(e) => {
            eprintln!("tinyview: failed to create window: {e}");
            return ExitCode::from(1);
        }
    };

    let _webview = match WebViewBuilder::new()
        .with_html(html)
        .build(&window)
    {
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
