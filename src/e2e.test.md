# e2e.rs テスト

`--allow-*` 権限フラグを **実際の WebView** で検証する live self-test harness（issue #5、`e2e` feature でのみコンパイル）。
webview.rs のユニットテストは CSP 文字列までしか見られないので、ブラウザが本当にその挙動を守るかをここで確かめる。
`EventLoop` を1回だけ `run_return` で回し、ステートマシンで各シナリオを順次実行。window+webview を1つずつ build → probe を流し → 破棄して次へ進む。window は不可視。各 probe は `<body>` 末尾の inline `<script>` で、`<head>` の CSP `<meta>` が効いた後に走り、結果は一時 IPC で読み戻す。ハードチェックがすべて通れば exit 0、失敗があれば 1。プラットフォーム依存の soft チェックは `WARN` 止まりで落とさない。

## 検証事項

|検証事項|内容|備考|
|---|---|---|
|デフォルト CSP が fetch をブロック|`connect-src 'none'` 違反が発火 → `blocked`。|--allow-fetch / hard|
|フラグで fetch 許可|違反は発火せず `allowed`。|--allow-fetch / hard|
|デフォルトでは `navigator.clipboard` を露出しない|macOS は neutralization init script（PRD §19.3）で保証＝hard、他 OS は WebKit port 依存＝soft。|--allow-clipboard|
|opaque origin の in-memory HTML では clipboard は露出しない|`with_html`（base URL 無し）で document が opaque origin になり、secure-context ゲートで Clipboard API は `--allow-clipboard` でも出ない。将来露出させる変更が入れば WARN として表面化。|--allow-clipboard / soft・既知の制約|
|デフォルトは実行をまたいで永続しない|ephemeral 原則。前の実行が書いた値を次の webview が読めてはいけない（`get:42` にならない）。|--allow-storage / hard・全 OS|
|opaque origin の in-memory HTML では localStorage が使えない|opaque origin のため `localStorage` が `SecurityError`。`--allow-storage` でも永続化できない。将来変われば WARN になる。|--allow-storage / soft・既知の制約|
