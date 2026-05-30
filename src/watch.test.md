# watch.rs テスト

`--watch` のファイル変更検知で使うパス比較を検証する。
`recompose_html` はユニットテスト困難（`HandlerContext` が `EventLoopProxy` を持ち、tao のイベントループはメインスレッド必須）なため、`--watch` の手動実行（PRD §9.10）と main.rs 経由の統合で end-to-end に検証する。

## 検証事項

|検証事項|内容|備考|
|---|---|---|
|直接の同値|同じ `PathBuf` 同士は一致。|paths_match|
|canonicalize で正規化して一致|`.` セグメントを含む冗長なパス（例 `/tmp/./file.html`）でも、canonicalize して比較すれば一致する。|paths_match|
