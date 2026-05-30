# webview.rs テスト

CSP（Content-Security-Policy）の構築と、HTML への `<meta>` 注入を検証する。

## build_csp — 権限フラグに応じた CSP 文字列

- **デフォルトは connect をブロック** — `connect-src 'none'`。あわせて `default-src 'self' 'unsafe-inline' data: blob:`、`object-src 'none'`、`base-uri 'none'`、`form-action 'none'` を含む。
- **`--allow-fetch` で connect を開放** — `connect-src https: http: ws: wss:` になり、`'none'` は消える。

## inject_csp — `<meta>` の挿入位置

- **`<head>` 直後に挿入** — `<meta http-equiv="Content-Security-Policy"` が `<head>` の直後・`<title>` より前に来る。
- **属性付き `<head>` でも正しく処理** — `<head lang="en">` のような開始タグの直後に挿入。
- **`<head>` が無ければ生成して挿入** — `<head>` 不在の HTML には head を補ってその中に meta を入れる。
- **allow-fetch を反映** — 注入される CSP も `connect-src https: http: ws: wss:` を含む。

## write_builtin_render_fixtures（`#[ignore]`、dev 用）

組み込みテンプレ（markdown / code / mermaid）を lib インライン + data 注入 + CSP 適用済みの状態で temp dir に書き出す手動ツール。
CSP を強制するブラウザで `connect-src 'none'` / `'unsafe-eval'` なしでも実際に描画できるかを目視確認するためのもの。
通常はスキップ。実行は:

```sh
cargo test --release write_builtin_render_fixtures -- --ignored --nocapture
```
