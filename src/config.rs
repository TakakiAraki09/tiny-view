//! `~/.tinyview/config.toml` の lazy loader。
//!
//! raw path (stdin + `-t` / `--param` / path どれも未指定) では **呼ばれない** 設計。
//! 呼ばれた場合のみ `OnceCell` を用いてプロセス内で1回だけファイルを読み込み、結果をキャッシュする。
//!
//! ファイル不在は `None` を silent に返す（warn 不要）。
//! TOML 構文エラーは stderr に warn を出した上で `None` を返す。

use std::cell::OnceCell;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(serde::Deserialize, Default, Debug)]
pub struct Config {
    pub window_width: Option<u32>,
    pub window_height: Option<u32>,
    pub default_template: Option<String>,
    #[serde(default)]
    pub extension: HashMap<String, String>,
    #[serde(default)]
    pub templates: HashMap<String, TemplateEntry>,
}

#[derive(serde::Deserialize, Default, Debug)]
pub struct TemplateEntry {
    #[serde(default)]
    pub params: HashMap<String, String>,
}

/// Resolve the TinyView config root directory (PRD §11.1).
///
/// 優先順位（先に **存在する** ディレクトリを採用）:
///   1. `$XDG_CONFIG_HOME/tinyview/`（`XDG_CONFIG_HOME` が設定されている場合）
///   2. `$HOME/.config/tinyview/`（XDG デフォルト）
///   3. `$HOME/.tinyview/`（後方互換: 旧来のデフォルト）
///
/// どの候補も存在しない場合は、後方互換のため `$HOME/.tinyview/` を最終
/// fallback として返す（既存ユーザーの挙動を変えないため）。`HOME` も
/// `XDG_CONFIG_HOME` も無ければ `None`。
///
/// config.toml と templates/ で同じ root を共有するため、ここに一元化する。
pub fn config_root() -> Option<PathBuf> {
    let xdg = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(|v| PathBuf::from(v).join("tinyview"));
    let home_config = std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/tinyview"));
    let legacy = std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".tinyview"));

    // 存在する最初の候補を採用。
    let candidates = [xdg, home_config, legacy.clone()];
    for cand in candidates.into_iter().flatten() {
        if cand.is_dir() {
            return Some(cand);
        }
    }
    // どれも存在しなければ legacy を最終 fallback とする（後方互換）。
    legacy
}

fn config_path() -> Option<PathBuf> {
    config_root().map(|r| r.join("config.toml"))
}

/// ロードしてキャッシュ。
/// - File 不在: silent `None`
/// - TOML パースエラー: stderr に warn を出し `None`
/// - 成功: `Some(&Config)` を返す
pub fn load_if_needed(cache: &OnceCell<Option<Config>>) -> Option<&Config> {
    cache
        .get_or_init(|| {
            let path = config_path()?;
            let bytes = std::fs::read_to_string(&path).ok()?;
            match toml::from_str::<Config>(&bytes) {
                Ok(c) => Some(c),
                Err(e) => {
                    eprintln!("tinyview: warn: invalid config.toml: {e}");
                    None
                }
            }
        })
        .as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::OnceCell;
    use std::io::Write;

    /// テスト用に `HOME` を一時 dir に差し替え、その中の `.tinyview/config.toml` に書き込む。
    /// `HOME` の変更はプロセスグローバルな環境変数操作なので、同時並行で走るテストが衝突しないよう
    /// `serial_test` 等を入れる選択肢もあるが、最小依存のため Mutex で排他する。
    static HOME_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn with_home<F: FnOnce(&std::path::Path)>(write_config: Option<&str>, body: F) {
        let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().expect("tempdir");
        let dot = tmp.path().join(".tinyview");
        std::fs::create_dir_all(&dot).expect("mkdir .tinyview");
        if let Some(s) = write_config {
            let mut f = std::fs::File::create(dot.join("config.toml")).expect("create config");
            f.write_all(s.as_bytes()).expect("write config");
        }

        // SAFETY: テスト中のみ HOME を書き換える。HOME_LOCK で他テストの並行アクセスを抑止している。
        let prev = std::env::var_os("HOME");
        // XDG_CONFIG_HOME が環境に漏れていると config_root() がそちらを優先しうるため、
        // この helper では未設定にして必ず tmp 配下の `.tinyview` を解決させる。
        let prev_xdg = std::env::var_os("XDG_CONFIG_HOME");
        // std::env::set_var は 2024 edition で unsafe 扱いだが現プロジェクトは edition=2021
        std::env::set_var("HOME", tmp.path());
        std::env::remove_var("XDG_CONFIG_HOME");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(tmp.path())));
        match prev {
            Some(p) => std::env::set_var("HOME", p),
            None => std::env::remove_var("HOME"),
        }
        match prev_xdg {
            Some(p) => std::env::set_var("XDG_CONFIG_HOME", p),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        if let Err(e) = result {
            std::panic::resume_unwind(e);
        }
    }

    #[test]
    fn returns_none_when_config_absent() {
        with_home(None, |_| {
            let cache: OnceCell<Option<Config>> = OnceCell::new();
            assert!(load_if_needed(&cache).is_none());
            // 2回目の呼び出しもキャッシュから None
            assert!(load_if_needed(&cache).is_none());
        });
    }

    #[test]
    fn returns_none_on_invalid_toml() {
        // stderr に warn が出る (capture せず) が、戻り値は None
        with_home(Some("this is = not = valid = toml ==="), |_| {
            let cache: OnceCell<Option<Config>> = OnceCell::new();
            assert!(load_if_needed(&cache).is_none());
        });
    }

    #[test]
    fn parses_valid_config() {
        let toml = r#"
window_width = 1200
window_height = 800
default_template = "raw"

[extension]
md = "markdown"
rs = "code"

[templates.markdown.params]
theme = "github"

[templates.code.params]
theme = "github-dark"
line_numbers = "true"
"#;
        with_home(Some(toml), |_| {
            let cache: OnceCell<Option<Config>> = OnceCell::new();
            let cfg = load_if_needed(&cache).expect("config should load");
            assert_eq!(cfg.window_width, Some(1200));
            assert_eq!(cfg.window_height, Some(800));
            assert_eq!(cfg.default_template.as_deref(), Some("raw"));
            assert_eq!(
                cfg.extension.get("md").map(String::as_str),
                Some("markdown")
            );
            assert_eq!(cfg.extension.get("rs").map(String::as_str), Some("code"));
            assert_eq!(
                cfg.templates
                    .get("markdown")
                    .and_then(|t| t.params.get("theme"))
                    .map(String::as_str),
                Some("github")
            );
            assert_eq!(
                cfg.templates
                    .get("code")
                    .and_then(|t| t.params.get("line_numbers"))
                    .map(String::as_str),
                Some("true")
            );
        });
    }

    /// `config_root()` の fallback chain (PRD §11.1) を 3 分岐でカバーする。
    /// `HOME` と `XDG_CONFIG_HOME` はプロセスグローバルなので `HOME_LOCK` で排他し、
    /// 終了時に必ず元へ戻す。
    fn with_env<F: FnOnce(&std::path::Path)>(home_subdirs: &[&str], xdg: Option<&str>, body: F) {
        let _guard = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().expect("tempdir");
        for d in home_subdirs {
            std::fs::create_dir_all(tmp.path().join(d)).expect("mkdir subdir");
        }

        let prev_home = std::env::var_os("HOME");
        let prev_xdg = std::env::var_os("XDG_CONFIG_HOME");
        std::env::set_var("HOME", tmp.path());
        match xdg {
            // xdg が tmp 配下の相対パスなら絶対パスへ。"" は「設定だが空」を表現。
            Some("") => std::env::set_var("XDG_CONFIG_HOME", ""),
            Some(rel) => std::env::set_var("XDG_CONFIG_HOME", tmp.path().join(rel)),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(tmp.path())));

        match prev_home {
            Some(p) => std::env::set_var("HOME", p),
            None => std::env::remove_var("HOME"),
        }
        match prev_xdg {
            Some(p) => std::env::set_var("XDG_CONFIG_HOME", p),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        if let Err(e) = result {
            std::panic::resume_unwind(e);
        }
    }

    #[test]
    fn config_root_prefers_xdg_when_set_and_present() {
        // 3 候補すべて存在させても XDG が最優先。
        with_env(
            &[".config/tinyview", ".tinyview", "xdg/tinyview"],
            Some("xdg"),
            |home| {
                assert_eq!(config_root(), Some(home.join("xdg/tinyview")));
            },
        );
    }

    #[test]
    fn config_root_falls_back_to_dot_config() {
        // XDG 未設定 → $HOME/.config/tinyview を採用。
        with_env(&[".config/tinyview", ".tinyview"], None, |home| {
            assert_eq!(config_root(), Some(home.join(".config/tinyview")));
        });
    }

    #[test]
    fn config_root_falls_back_to_legacy_dot_tinyview() {
        // XDG 未設定 & .config 不在 → 後方互換の $HOME/.tinyview を採用。
        with_env(&[".tinyview"], None, |home| {
            assert_eq!(config_root(), Some(home.join(".tinyview")));
        });
    }

    #[test]
    fn config_root_skips_nonexistent_xdg_dir() {
        // XDG は設定されているが当該 dir が無い → 存在する .tinyview へフォールバック。
        with_env(&[".tinyview"], Some("xdg"), |home| {
            assert_eq!(config_root(), Some(home.join(".tinyview")));
        });
    }

    #[test]
    fn config_root_defaults_to_legacy_when_none_exist() {
        // どの候補も存在しない場合でも legacy path を最終 fallback として返す。
        with_env(&[], None, |home| {
            assert_eq!(config_root(), Some(home.join(".tinyview")));
        });
    }

    #[test]
    fn config_root_ignores_empty_xdg() {
        // XDG_CONFIG_HOME="" は未設定扱い。
        with_env(&[".config/tinyview"], Some(""), |home| {
            assert_eq!(config_root(), Some(home.join(".config/tinyview")));
        });
    }
}
