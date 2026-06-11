# webview.rs テスト

CSP（Content-Security-Policy）の構築と、HTML への `<meta>` 注入を検証する。

## 検証事項

|検証事項|内容|備考|
|---|---|---|
|デフォルトは connect をブロック|`connect-src 'none'`。あわせて `default-src 'self' 'unsafe-inline' data: blob:`、`object-src 'none'`、`base-uri 'none'`、`form-action 'none'` を含む。|build_csp|
|`allow_fetch`（meta 由来の実効許可）で connect を開放|`connect-src https: http: ws: wss:` になり、`'none'` は消える。fetch 許可の供給源は HTML の `<meta name="tinyview-allow" content="fetch">` のみ（CLI フラグは廃止）。|build_csp|
|`<head>` 直後に挿入|`<meta http-equiv="Content-Security-Policy"` が `<head>` の直後・`<title>` より前に来る。|inject_csp|
|属性付き `<head>` でも正しく処理|`<head lang="en">` のような開始タグの直後に挿入。|inject_csp|
|`<head>` が無ければ生成して挿入|`<head>` 不在の HTML には head を補ってその中に meta を入れる。|inject_csp|
|allow_fetch を反映|注入される CSP も `connect-src https: http: ws: wss:` を含む。|inject_csp|
|meta による fetch 許可を検出|`<meta name="tinyview-allow" content="fetch">` を検出（単/二重引用符・大文字小文字・トークンリスト対応）。`prefetch` 等の部分一致や他の `name` は誤検出しない。|meta_allows_fetch|
|meta 許可を実効 perms に折り込む|meta があれば `allow_fetch` が立つ（fetch 許可の唯一の供給源）。事前に立っている `allow_fetch` はそのまま通る（OR）。clipboard / storage は meta では許可されない（CLI のみ）。|effective_perms|
|meta 許可が CSP に反映される|meta 付き HTML を prepare すると `connect-src https: http: ws: wss:` になる。raw mode でも meta 許可があれば CSP 注入が走る。許可なしの raw mode は注入しない。|prepare_html|
