# main.rs テスト

CLI 引数のパース（clap）と、params のマージ規則を検証する。

## CLI フラグのパース

- **デフォルトは frameless / transparent ともに false**。
- **`--frameless` を解釈** — frameless=true、transparent は false のまま。
- **`--transparent` を解釈** — transparent=true、frameless は false のまま。
- **両方同時指定** — frameless と transparent がともに true。
- **既存フラグと共存（リグレッションガード）** — `--frameless --transparent` を追加しても `--width` / `--height` / `--allow-fetch` / `--foreground` のパースを壊していないことを確認。

## merge_params — config と CLI の params 合成

- **CLI が config を上書きする** — config の `[templates.code.params] lang = "python"` を `--param lang=rust` が上書き。一方で上書きされない config 由来の `theme=github` はそのまま残る。
