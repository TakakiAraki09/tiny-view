# TinyView Template 自作ガイド

このドキュメントは、TinyView 用の **自作 template** を作るための実装者向けガイドです。
仕様の最終的な正典は [`docs/PRD.md`](../../docs/PRD.md)（特に §13 / §14 / §15 / §19）です。
本ガイドはそれを「template を書く側」の視点で噛み砕いたものです。

本ガイドは `tinyview --guide template` でいつでも標準出力に表示できます。
AI agent に template を生成させる際のコンテキストとしても、そのまま渡して使えます。

---

## 1. TinyView の template とは何か

TinyView は **template engine を持ちません**。minijinja / tera / handlebars のような
プレースホルダ文法やループ構文は一切ありません。

template の正体は **自己完結した単一の HTML ファイル**です。runtime がやることは、
その HTML 文字列に対して **たった 1 回の `str::replace`** を実行し、
所定のマーカーを `window.__TINYVIEW__` の JSON literal に差し替えるだけです。

```text
template.html  ──(マーカーを JSON literal に1回置換)──▶  最終 HTML 文字列  ──▶  WebView へ注入
```

- 描画ロジック（markdown のパース、HTML escape、syntax highlight 等）は **すべて template 側の JS の責任**です。Rust 側は何も解釈しません。
- 入力は最終的に **メモリ上の単一 HTML 文字列**に合成され、一時 HTML ファイルは生成されません。
- ウィンドウを閉じれば DOM / JS state は全破棄されます（ephemeral）。永続状態を持ち込んではいけません。

---

## 2. 最小の template ひな形

template には、`<head>` 内のどこか 1 箇所に **データ注入マーカー**を含めます。
マーカー文字列は実装の `MARKER` 定数（`src/template.rs`）と**完全一致**している必要があります。

マーカー（この文字列そのものが 1 回だけ置換される）:

```text
/*__TINYVIEW__*/ null /*__TINYVIEW__*/
```

慣習的には次の `<script>` 行として書きます（PRD §14.1 と同一）:

```html
<script>window.__TINYVIEW__ = /*__TINYVIEW__*/ null /*__TINYVIEW__*/;</script>
```

これを踏まえた最小の template ひな形:

```html
<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="color-scheme" content="light dark">
<title>tinyview</title>
<style>
  :root{color-scheme:light dark}
  html,body{margin:0;padding:16px;background:Canvas;color:CanvasText}
</style>
<!-- ↓ このマーカー1箇所が JSON literal に置換される -->
<script>window.__TINYVIEW__ = /*__TINYVIEW__*/ null /*__TINYVIEW__*/;</script>
<script>
(function(){
  var d = window.__TINYVIEW__ || {};
  if (d.title) document.title = d.title;
  function run(){
    var el = document.createElement("pre");
    // textContent で安全に注入（HTML として解釈させない = escape は template の責任）
    el.textContent = (d.input == null) ? "" : String(d.input);
    document.body.appendChild(el);
  }
  if (document.body) { run(); }
  else { document.addEventListener("DOMContentLoaded", run, { once:true }); }
})();
</script>
</head>
<body></body>
</html>
```

> 置換の挙動について（`src/template.rs` の `substitute_marker()`）:
> - マーカーは **最初に見つかった 1 箇所だけ**置換されます。
> - **マーカーが存在しない場合**は、stderr に警告
>   （`tinyview: warning: template has no ... marker; injection skipped`）を出したうえで、
>   HTML をそのまま使います（エラーにはなりません）。この場合 `window.__TINYVIEW__` は
>   `null` のままになるので、JS 側で `window.__TINYVIEW__ || {}` のようにガードしておくと安全です。

---

## 3. `window.__TINYVIEW__` のスキーマ

runtime が注入する JSON literal は次の構造です。フィールド名・型は
`src/template.rs` の `build_literal()`（`serde_json::json!` ブロック）から正確に読み取っています。

```ts
window.__TINYVIEW__ = {
  input:  string,                  // 入力本体（stdin / file / --html のいずれか）
  params: Record<string, string>,  // マージ済みパラメータ（後述。値は常に string）
  title:  string,                  // ウィンドウタイトル
  path:   string | null,           // file 入力時のみファイルパス文字列。stdin / inline は null
};
```

| フィールド | 型 | 意味 |
| ---------- | -- | ---- |
| `input`    | `string` | 表示対象の本文。stdin・ファイル・`--html` のどれかから来た生の文字列。 |
| `params`   | `Record<string, string>` | `[templates.<name>.params]`（config）と `--param key=value`（CLI）をマージした結果。**キー・値ともに常に文字列**（数値や真偽値が必要なら template 側 JS でパースする）。デフォルトは空オブジェクト相当。 |
| `title`    | `string` | ウィンドウ／ドキュメントのタイトル。`document.title = d.title` で反映するのが慣習。 |
| `path`     | `string \| null` | ファイル入力のときだけそのパス文字列。stdin / `--html` インライン入力では `null`。 |

> 注意: `params` の値は TOML / CLI 由来でも **すべて文字列**になります。
> 例えば `line_numbers = "true"` は JS では `"true"`（文字列）として届くので、
> `d.params.line_numbers === "true"` のように比較してください。

---

## 4. 自作 template を作って使うまで（ステップバイステップ）

### Step 1. template ファイルを置く

自作 template は **config root 配下の `templates/` ディレクトリ**に
`<name>.html` として置きます。

```text
<config root>/templates/<name>.html
```

`<config root>` の解決順（先に **存在する** ディレクトリが採用される。`src/config.rs` の `config_root()`）:

1. `$XDG_CONFIG_HOME/tinyview/`（環境変数 `XDG_CONFIG_HOME` が空でなく設定されている場合）
2. `$HOME/.config/tinyview/`（XDG デフォルト）
3. `$HOME/.tinyview/`（後方互換: 旧来のデフォルト）

どの候補も存在しない場合は、後方互換のため最終 fallback として `$HOME/.tinyview/` が使われます。

例（旧来のデフォルトを使う場合）:

```bash
mkdir -p ~/.tinyview/templates
# Step 2 のひな形を保存
$EDITOR ~/.tinyview/templates/mybox.html
```

> template 名と組み込み名の衝突に注意（`src/template.rs` の `name_to_ref()`）。
> `raw` / `text` / `minimal` / `markdown` / `mermaid` / `code` は**組み込み template として予約**されており、
> これらの名前を `-t` に渡しても `templates/` 配下のファイルではなく組み込み版が使われます。
> 自作 template にはこれら以外の名前を付けてください。それ以外の名前は
> `templates/<name>.html` を読むユーザー template として解決されます。

### Step 2. config.toml で拡張子マッピング / default / params を設定する

`<config root>/config.toml` を編集します（`src/config.rs` の `Config` / `TemplateEntry` 構造に対応）。
このファイルは raw path 以外でのみ lazy にロードされます。不在でもエラーにはなりません。

```toml
# ウィンドウサイズ（任意）
window_width  = 1200
window_height = 800

# template を一切明示しなかったときのデフォルト
default_template = "raw"

# 拡張子 → template 名 のマッピング（ファイル入力時に使われる）
[extension]
md  = "markdown"
rs  = "code"
box = "mybox"      # 例: *.box ファイルは自作 mybox template で開く

# template ごとのデフォルト params
# テーブル名のキーは template 名（自作 template ならファイル名から .html を除いたもの）
[templates.mybox.params]
accent = "teal"
mode   = "compact"

[templates.code.params]
theme = "github-dark"
```

- `[extension]` のキーは拡張子（先頭ドット無し）。マッチは大文字小文字を区別しません（`.MD` でも `md` にマッチ）。
- `[templates.<name>.params]` の `<name>` は template 名です。自作 template の場合は **ファイル名から `.html` を除いた stem** がそのまま名前になります（`mybox.html` → `mybox`）。

### Step 3. `--template` / `-t` で明示指定する

```bash
cat input.txt | tinyview -t mybox
cat input.txt | tinyview --template mybox
```

明示指定は他のすべて（拡張子マッピング・`default_template`）より優先されます。

template 解決の優先順位（PRD §13 / `src/template.rs` の `resolve()`）:

1. 明示指定 `--template` / `-t`
2. 拡張子マッピング（config の `[extension]`、ファイル入力時のみ）
3. `default_template`（config）
4. `raw`（フォールバック）

> 入力解決の優先順位は別軸で **stdin > ファイルパス引数 > `--html` インライン** です。

### Step 4. params を `--param key=value` で渡す

```bash
cat input.txt | tinyview -t mybox --param accent=crimson --param mode=full
```

`--param` は繰り返し指定できます。フォーマットは `key=value`（最初の `=` で分割）。

config の `[templates.<name>.params]` と CLI の `--param` は **マージ**され、
**同じキーがある場合は CLI が勝ちます**（`src/main.rs` の `merge_params()`）。
template 側からは結果が `window.__TINYVIEW__.params` として見えます。

例: config に `accent = "teal"`、CLI で `--param accent=crimson` を渡すと、
template には `params.accent === "crimson"` が届きます。

> raw mode の注意（PRD §13.1）: `raw` が選ばれた場合は最速パスのため
> config 読み込み・`--param` 評価・marker 置換がすべて skip されます。
> **`--param` を効かせたいなら `text` / `minimal` / 自作 template を明示してください。**

---

## 5. 絶対に守るべき制約

これらは TinyView の core 原則（No Server / Ephemeral / Native WebView）に直結します。違反すると template が壊れる、または起動目標を損ないます。

### 5.1 外部リソース参照の禁止（最重要）

TinyView は **サーバを一切持たない**ため、外部 URL を解決できません。
さらに runtime はデフォルトで CSP `connect-src 'none'`（§19.3）を注入します。

禁止:

```html
<!-- すべて NG: 外部 URL からの読み込みは解決できない -->
<link rel="stylesheet" href="https://cdn.example.com/style.css">
<script src="https://cdn.example.com/lib.js"></script>
```

ライブラリ・CSS・フォントが必要なら、**template 内に inline で同梱（vendor）**します。

組み込み `markdown` template の実例（`src/templates/markdown.html`）では、marked.js と
highlight.js を **`<script src>` ではなく `<script>...</script>` の中身として inline** しています。
ビルド時に runtime が次のプレースホルダを vendor 済みライブラリ本体へ置換しています:

```html
<style>
/*__TINYVIEW_HLJS_CSS__*/   <!-- highlight.js のテーマ CSS が丸ごと入る -->
</style>
<script>/*__TINYVIEW_MARKED__*/</script>   <!-- marked.min.js の中身が丸ごと入る -->
<script>/*__TINYVIEW_HLJS__*/</script>      <!-- highlight.min.js の中身が丸ごと入る -->
```

> この `/*__TINYVIEW_<LIB>__*/` 方式は **組み込み template 専用の仕組み**です
> （vendor 済みライブラリはバイナリに `include_str!` で同梱されている）。
> 自作 template では使えないので、自分で使うライブラリの中身を `<script>...</script>` に
> 直接コピペして同梱してください。同梱物の選定・ライセンス管理は template 作者の責任です。
> （vendor の進め方の参考: `src/templates/vendor/README.md`。
> 同梱する minified JS が `</script>` / `</style>` 部分文字列を含まないこと、
> マーカー文字列 `/*__TINYVIEW__*/ null /*__TINYVIEW__*/` を含まないことを確認すること。）

### 5.2 HTML escape は template 側 JS の責任

`input` は信頼できない可能性のある生文字列です。プレーンテキストとして見せたいなら
`element.textContent = d.input`（`text` template 方式）を使い、HTML として解釈させないこと。
`innerHTML` を使う場合（`minimal` 方式）は **信頼境界が呼び出し側にある**点を理解した上で使ってください。

### 5.3 CSP `<meta>` を template に書かない（§19.4）

CSP は runtime が排他的に管理します。**template HTML 側に
`<meta http-equiv="Content-Security-Policy">` を書いてはいけません。**
両方存在すると最も制限的な値が勝ち、template が想定外に壊れます。

### 5.4 永続状態を持たない（ephemeral）

localStorage / IndexedDB / Cookie 等の永続ストレージに依存しないこと。
デフォルトで incognito 相当のため永続せず、ウィンドウを閉じれば全破棄されます。

---

## 6. ネットワーク（fetch）が必要な template の権限許可

デフォルトでは runtime が CSP `connect-src 'none'` を注入するため、
**fetch / XHR / WebSocket はすべて遮断**されます（meta もフラグも無い状態）。

ネットワークが必要な template には、次の 2 つの許可経路があります。両者は **OR 関係**で、
どちらかがあれば許可されます。

### 6.1 template 側で宣言する `<meta name="tinyview-allow">`

template の `<head>` に次の meta を置くと、その template は CLI で `--allow-fetch` を
付けなくても fetch / XHR / WebSocket が許可されます。

```html
<head>
  <meta name="tinyview-allow" content="fetch">
  ...
</head>
```

- `content` は **スペース区切り**のトークン列です（例: `content="fetch"`）。
- 現状認識されるトークンは **`fetch` のみ**です。

これは「この template は本質的にネットワークを必要とする」という宣言を template 自身に
持たせる仕組みで、毎回 CLI フラグを付けなくてよくするためのものです。

### 6.2 CLI フラグ `--allow-fetch`

```bash
cat input.json | tinyview -t my-fetch-template --allow-fetch
```

`--allow-fetch` を付けると、runtime は CSP の `connect-src` を
`'none'` から `https: http: ws: wss:` に緩和します（`src/webview.rs` の `build_csp()`）。

### 6.3 許可の関係まとめ

| `<meta name="tinyview-allow" content="fetch">` | `--allow-fetch` | 結果 |
| :--: | :--: | ---- |
| 無し | 無し | `connect-src 'none'`（全ネットワーク遮断、デフォルト） |
| 有り | 無し | fetch 許可 |
| 無し | 有り | fetch 許可 |
| 有り | 有り | fetch 許可 |

> 注意: それでも **No Server 原則**は変わりません。許可されるのは template 内 JS からの
> 外向き通信であって、TinyView がローカルにサーバを立てるわけではありません。

---

## 7. 動作確認

stdin から流し込んで自作 template で開く基本形:

```bash
# 自作 mybox template で確認
cat mytemplate-input.txt | tinyview -t mybox

# params を渡して確認
cat mytemplate-input.txt | tinyview -t mybox --param accent=crimson

# 拡張子マッピング経由（config の [extension] box = "mybox" を設定済みの場合）
tinyview sample.box

# fetch を使う template の確認
cat data.json | tinyview -t my-fetch-template --allow-fetch
```

TinyView は非ブロッキング CLI なので、launch 後ただちに shell に制御が返ります。
前景に保持して挙動を見たい場合は `--foreground` を付けてください。

---

## 8. よくある落とし穴

- **マーカーの記述ミス**: `/*__TINYVIEW__*/ null /*__TINYVIEW__*/` は 1 文字でも違うと
  置換されません。スペース（`null` の前後）やアンダースコアの数（`__` は 2 個）に注意。
  置換されないと stderr に警告が出て `window.__TINYVIEW__` は `null` のままになります。
- **外部 URL 参照**: `<script src="https://...">` / `<link href="https://...">` は
  サーバ非依存 + CSP `connect-src 'none'` により読み込めません。inline 同梱に置き換える。
- **HTML escape 忘れ**: `innerHTML = d.input` は入力を HTML として解釈・実行します。
  プレーン表示なら `textContent` を使う。
- **CSP を template に書く**: `<meta http-equiv="Content-Security-Policy">` を書くと
  runtime 注入分と衝突して壊れます（§19.4）。書かない。
- **params の型を勘違いする**: `params` の値は常に文字列。`"true"` / `"3"` のように届くので
  JS 側で明示的にパース・比較する。
- **raw で `--param` が効かない**: raw mode は最速パスとして param 評価を skip する。
  param を使うなら `text` / `minimal` / 自作 template を明示する。
- **組み込み名と衝突**: `raw` / `text` / `minimal` / `markdown` / `mermaid` / `code` という
  名前の `templates/<name>.html` を置いても読まれない（組み込みが優先）。別名にする。
- **params テーブルのキー名ミス**: 自作 template の `[templates.<name>.params]` の `<name>` は
  ファイル名から `.html` を除いた stem。`mybox.html` なら `[templates.mybox.params]`。
- **fetch がブロックされる**: meta（`<meta name="tinyview-allow" content="fetch">`）も
  `--allow-fetch` も無いと `connect-src 'none'` で遮断される。どちらかを付ける。
