# config.rs テスト

`config.toml` の読み込みと、設定ディレクトリ解決（fallback chain, PRD §11.1）を検証する。
`HOME` / `XDG_CONFIG_HOME` をテスト中だけ一時 dir に差し替えるため、`HOME_LOCK` Mutex で並行テストの衝突を防いでいる。
設定ディレクトリの優先順位は XDG → `~/.config/tinyview` → 後方互換の `~/.tinyview`。

## 検証事項

|検証事項|内容|備考|
|---|---|---|
|config が無ければ None|`.tinyview/config.toml` 不在なら None。2回目もキャッシュから None。|load_if_needed|
|不正な TOML は None|パース失敗時は stderr に warn を出しつつ戻り値は None（落とさない）。|load_if_needed|
|正しい config をパースできる|`window_width/height`、`default_template`、`[extension]` マッピング、`[templates.<name>.params]` がすべて期待通り読める。|load_if_needed|
|XDG が設定され存在すれば最優先|3候補すべて存在しても XDG を選ぶ。|config_root|
|XDG 未設定なら .config を採用|`$HOME/.config/tinyview` を採用。|config_root|
|.config も無ければ legacy へ|`$HOME/.tinyview`（後方互換）へフォールバック。|config_root|
|XDG dir が存在しなければスキップ|XDG は設定済みだが dir 不在 → 存在する `.tinyview` へフォールバック。|config_root|
|どの候補も無くても legacy を返す|最終 fallback として `.tinyview` path を返す。|config_root|
|空の XDG は未設定扱い|`XDG_CONFIG_HOME=""` は無視して `.config` を採用。|config_root|
