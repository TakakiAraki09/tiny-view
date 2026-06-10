//! Detach-by-default via re-exec + stdin pipe.
//!
//! ## Why not `fork()`?
//!
//! macOS は `fork()` した子プロセスに対し、WindowServer / Quartz への
//! Mach port を引き継がせない。`fork()+setsid()+exec無し` で WebView を
//! 起動しても **ウィンドウは描画されない**（lsof で IOSurface / WindowServer
//! 接続が無いことを確認済み）。これは Cocoa fork-safety 一般の問題ではなく、
//! 「子プロセスが responsible process として WindowServer に再接続できない」
//! という macOS 固有の制約。
//!
//! ## 採用方式
//!
//! `Command::new(current_exe).spawn()` で **自身を新しいプロセスとして exec** し、
//! 親プロセスは即 `exit(0)` する。子は fresh な Mach context を持つため、
//! AppKit / WindowServer に正しく接続できる。
//!
//! 合成済み HTML は子の stdin に pipe で書き込む（一時ファイルを作らないため
//! PRD §6.3 "No Generated Preview File" を守る）。
//!
//! 子は env var `TINYVIEW_DETACHED_CHILD=1` で識別。子は `Cli::parse()` の後、
//! 通常の入力解決 / template runtime を **skip** し、stdin から事前合成済み HTML
//! を読み込んで直接 WebView を起動する。
//!
//! ## SIGHUP 対策
//!
//! `pre_exec` で `setsid()` を呼んで子を新しい session leader にする。これで
//! 親シェル終了時の SIGHUP が controlling terminal 経由で届かない。`setsid` は
//! `exec` 前に呼ぶので、fork-after-Cocoa の問題は発生しない（exec で Mach
//! state が wipe される）。
//!
//! ## Windows
//!
//! Windows には session / SIGHUP の概念が無いため、`CommandExt::creation_flags`
//! でプロセス生成フラグを明示する（PRD §6.7）:
//!
//! - `DETACHED_PROCESS` — 子を親のコンソールから切り離す。これにより親シェル
//!   終了後も子が生存し、かつ子が新しいコンソールウィンドウを作らない
//!   （＝コンソールのチラつきを防ぐ）。
//! - `CREATE_NEW_PROCESS_GROUP` — 子を新しいプロセスグループのルートにし、
//!   親グループ宛ての Ctrl+C / Ctrl+Break が子に伝播しないようにする。
//!
//! これらは安定した Win32 ABI 定数なので、`windows-sys` 等の依存を増やさず
//! （バイナリサイズ予算 <10MB を守るため）リテラルで定義する。

use std::io::Write;
use std::process::{Command, Stdio};

/// Win32 `DETACHED_PROCESS` プロセス生成フラグ。
/// 子を親のコンソールから切り離す（新しいコンソールも作らない）。
#[cfg(windows)]
const DETACHED_PROCESS: u32 = 0x0000_0008;

/// Win32 `CREATE_NEW_PROCESS_GROUP` プロセス生成フラグ。
/// 子を新しいプロセスグループのルートにする。
#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

/// 子プロセスを識別する env var。値は `1`。
pub const DETACHED_CHILD_ENV: &str = "TINYVIEW_DETACHED_CHILD";

/// 子に raw mode かどうかを伝える env var（CLI 引数では表現しないため）。
const RAW_MODE_ENV: &str = "TINYVIEW_RAW_MODE";

pub struct SpawnOpts<'a> {
    pub html: &'a str,
    pub width: u32,
    pub height: u32,
    pub raw_mode: bool,
    // NOTE: fetch permission is intentionally absent here. It is granted only
    // via `<meta name="tinyview-allow" content="fetch">` inside the composed
    // HTML (PRD §19.2.1), which travels through the stdin pipe — the child
    // re-derives it with `webview::effective_perms`.
    pub allow_clipboard: bool,
    pub allow_storage: bool,
    /// PRD §9.8 — pass `--frameless` to the detached child.
    pub frameless: bool,
    /// PRD §9.9 — pass `--transparent` to the detached child.
    pub transparent: bool,
}

/// 自身を detached child として spawn し、合成済み HTML を stdin に書き込む。
/// 戻り値 `Ok(())` 後、呼び出し側は即 `exit(0)` してよい。
pub fn spawn(opts: &SpawnOpts<'_>) -> std::io::Result<()> {
    let exe = std::env::current_exe()?;
    let mut cmd = Command::new(exe);

    cmd.env(DETACHED_CHILD_ENV, "1");
    if opts.raw_mode {
        cmd.env(RAW_MODE_ENV, "1");
    }
    cmd.arg("--width").arg(opts.width.to_string());
    cmd.arg("--height").arg(opts.height.to_string());
    if opts.allow_clipboard {
        cmd.arg("--allow-clipboard");
    }
    if opts.allow_storage {
        cmd.arg("--allow-storage");
    }
    if opts.frameless {
        cmd.arg("--frameless");
    }
    if opts.transparent {
        cmd.arg("--transparent");
    }

    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());

    // Unix: 子で setsid() を呼んで新セッションリーダーにする。
    // pre_exec は fork 後 exec 前に走るため、その時点での Cocoa state は無く安全。
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: pre_exec closure は fork-after / exec-before の制約下で動く。
        // setsid() は単一 syscall で malloc / lock を必要としないため安全。
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    // Windows: コンソールから切り離し、独立したプロセスグループで起動する。
    // PRD §6.7 — 親シェル終了後も子を生存させ、子コンソールのチラつきを防ぐ。
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }

    let mut child = cmd.spawn()?;

    // 合成済み HTML を stdin pipe に書き込む。drop で EOF を送る。
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(opts.html.as_bytes())?;
    }

    // `child` をそのまま drop。Rust の Child::drop は子を kill しない。
    // 親が exit 後、子は init/launchd に orphan-adopt されて reap される。
    Ok(())
}

/// このプロセスは parent が再 exec した detached child か？
pub fn is_detached_child() -> bool {
    std::env::var_os(DETACHED_CHILD_ENV).is_some()
}

/// 子プロセス側で raw mode かどうかを取得する。
pub fn detached_raw_mode() -> bool {
    std::env::var_os(RAW_MODE_ENV).is_some()
}
