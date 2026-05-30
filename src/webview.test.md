# webview.rs テスト

CSP（Content-Security-Policy）の構築と、HTML への `<meta>` 注入を検証する。

## 検証事項

|検証事項|内容|備考|
|---|---|---|
|デフォルトは connect をブロック|`connect-src 'none'`。あわせて `default-src 'self' 'unsafe-inline' data: blob:`、`object-src 'none'`、`base-uri 'none'`、`form-action 'none'` を含む。|build_csp|
|`--allow-fetch` で connect を開放|`connect-src https: http: ws: wss:` になり、`'none'` は消える。|build_csp|
|`<head>` 直後に挿入|`<meta http-equiv="Content-Security-Policy"` が `<head>` の直後・`<title>` より前に来る。|inject_csp|
|属性付き `<head>` でも正しく処理|`<head lang="en">` のような開始タグの直後に挿入。|inject_csp|
|`<head>` が無ければ生成して挿入|`<head>` 不在の HTML には head を補ってその中に meta を入れる。|inject_csp|
|allow-fetch を反映|注入される CSP も `connect-src https: http: ws: wss:` を含む。|inject_csp|
