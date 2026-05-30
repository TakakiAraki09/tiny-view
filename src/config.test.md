# config.rs テスト

`config.toml` の読み込みと、設定ディレクトリ解決（fallback chain, PRD §11.1）を検証する。
`HOME` / `XDG_CONFIG_HOME` をテスト中だけ一時 dir に差し替えるため、`HOME_LOCK` Mutex で並行テストの衝突を防いでいる。

## load_if_needed — config.toml の読み込み

- **config が無ければ None** — `.tinyview/config.toml` 不在なら None。2回目もキャッシュから None。
- **不正な TOML は None** — パース失敗時は stderr に warn を出しつつ戻り値は None（落とさない）。
- **正しい config をパースできる** — `window_width/height`、`default_template`、`[extension]` マッピング、`[templates.<name>.params]` がすべて期待通り読める。

## config_root — 設定ディレクトリの解決

優先順位は XDG → `~/.config/tinyview` → 後方互換の `~/.tinyview`。

- **XDG が設定され存在すれば最優先** — 3候補すべて存在しても XDG を選ぶ。
- **XDG 未設定なら .config を採用** — `$HOME/.config/tinyview`。
- **.config も無ければ legacy へ** — `$HOME/.tinyview`（後方互換）。
- **XDG dir が存在しなければスキップ** — XDG は設定済みだが dir 不在 → 存在する `.tinyview` へフォールバック。
- **どの候補も無くても legacy を返す** — 最終 fallback として `.tinyview` path を返す。
- **空の XDG は未設定扱い** — `XDG_CONFIG_HOME=""` は無視して `.config` を採用。
