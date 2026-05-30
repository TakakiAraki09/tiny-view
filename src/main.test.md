# main.rs テスト

CLI 引数のパース（clap）と、params のマージ規則を検証する。

## 検証事項

|検証事項|内容|備考|
|---|---|---|
|デフォルトはフラグ false|frameless / transparent ともに false。|CLI フラグのパース|
|`--frameless` を解釈|frameless=true、transparent は false のまま。|CLI フラグのパース|
|`--transparent` を解釈|transparent=true、frameless は false のまま。|CLI フラグのパース|
|両方同時指定|frameless と transparent がともに true。|CLI フラグのパース|
|既存フラグと共存|`--frameless --transparent` を追加しても `--width` / `--height` / `--allow-fetch` / `--foreground` のパースを壊さない。|リグレッションガード|
|CLI が config を上書きする|config の `[templates.code.params] lang = "python"` を `--param lang=rust` が上書き。上書きされない `theme=github` はそのまま残る。|merge_params|
