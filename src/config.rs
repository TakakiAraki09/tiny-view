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

fn config_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".tinyview/config.toml"))
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
        // std::env::set_var は 2024 edition で unsafe 扱いだが現プロジェクトは edition=2021
        std::env::set_var("HOME", tmp.path());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(tmp.path())));
        match prev {
            Some(p) => std::env::set_var("HOME", p),
            None => std::env::remove_var("HOME"),
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
            assert_eq!(cfg.extension.get("md").map(String::as_str), Some("markdown"));
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
}
