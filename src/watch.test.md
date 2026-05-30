# watch.rs テスト

`--watch` のファイル変更検知で使うパス比較を検証する。

## paths_match — 監視対象パスの一致判定

- **直接の同値** — 同じ `PathBuf` 同士は一致。
- **canonicalize で正規化して一致** — `.` セグメントを含む冗長なパス（例 `/tmp/./file.html`）でも、canonicalize して比較すれば一致する。

## 補足

`recompose_html` はユニットテスト困難（`HandlerContext` が `EventLoopProxy` を持ち、tao のイベントループはメインスレッド必須）。
こちらは `--watch` の手動実行（PRD §9.10）と main.rs 経由の統合で end-to-end に検証される。
