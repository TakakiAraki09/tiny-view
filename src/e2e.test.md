# e2e.rs テスト

`--allow-*` 権限フラグを **実際の WebView** で検証する live self-test харness（issue #5、`e2e` feature でのみコンパイル）。
webview.rs のユニットテストは CSP 文字列までしか見られないので、ブラウザが本当にその挙動を守るかをここで確かめる。

## 実行方法

```sh
# macOS（ローカル）
TINYVIEW_E2E_SELFTEST=1 cargo run --features e2e
# Linux CI（仮想ディスプレイ越し）
xvfb-run -a env TINYVIEW_E2E_SELFTEST=1 cargo run --features e2e
```

ハードチェックがすべて通れば exit 0、失敗があれば 1。プラットフォーム依存の soft チェックは `WARN` 止まりで落とさない。

## 仕組み

`EventLoop` を1回だけ `run_return` で回し、ステートマシンで各シナリオを順次実行。
window+webview を1つずつ build → probe を流し→破棄して次へ（storage の「実行をまたいだ」確認はこのテアダウンが前提）。
window は不可視。各 probe は `<body>` 末尾の inline `<script>` で、`<head>` の CSP `<meta>` が効いた後に走る。結果は一時 IPC で読み戻す。

## 検証する不変条件

### --allow-fetch
- **デフォルト CSP が fetch をブロック**（hard）— `connect-src 'none'` 違反が発火 → `blocked`。
- **フラグで fetch 許可**（hard）— 違反は発火せず `allowed`。

### --allow-clipboard
- **デフォルトでは `navigator.clipboard` を露出しない** — macOS は neutralization init script（PRD §19.3）で保証＝hard、他 OS は WebKit port 依存＝soft。
- **opaque origin の in-memory HTML では clipboard は露出しない**（soft / 既知の制約）— `with_html`（base URL 無し）のため document が opaque origin になり、secure-context ゲートで Clipboard API は `--allow-clipboard` でも出ない。将来露出させる変更が入れば WARN として表面化する。

### --allow-storage
- **デフォルトは実行をまたいで永続しない**（hard, 全 OS）— ephemeral 原則。前の実行が書いた値を次の webview が読めてはいけない（`get:42` にならない）。
- **opaque origin の in-memory HTML では localStorage が使えない**（soft / 既知の制約）— opaque origin のため `localStorage` が `SecurityError`。`--allow-storage` でも永続化できない。将来変われば WARN になる。
