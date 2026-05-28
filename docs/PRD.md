# TinyView PRD

**Version:** v0.1
**Status:** Draft Final
**Product Type:** CLI-first transient WebView runtime
**Core Value:** サーバーなし、ポートなし、物理プレビュー生成なしで、CLIから即座にWeb UIを表示する

---

# 1. 概要

TinyView は、HTML / CSS / JavaScript / Markdown / Mermaid / Code snippet などの入力を、CLIから即座に軽量WebView上で描画するための **一時実行型プレビューランタイム** である。

最重要価値は以下。

```text
入力
↓
即描画
↓
閉じたら消える
```

TinyView は、従来のフロントエンド開発環境のように以下を要求しない。

- localhost server
- port allocation
- dev server
- temp HTML file generation
- browser tab management
- project scaffold
- bundler startup
- npm install

最小体験は以下。

```bash
echo '<button onclick="alert(1)">OK</button>' | tinyview
```

実行すると、ネイティブWebViewの小さなウィンドウが立ち上がり、HTMLが即座に描画される。ウィンドウを閉じるとプロセスも終了し、実行時状態は破棄される。

---

# 2. プロダクトの核

TinyView は「軽量ブラウザ」ではない。

TinyView は：

```text
ephemeral rendering primitive
```

である。

つまり、Web UIを一時的に表示するための最小単位であり、開発サーバーやブラウザタブの代替ではない。

---

# 3. 解決したい課題

現在、少しだけHTMLやUIを確認したい場合でも、一般的には次のような手順が必要になる。

```text
ファイルを作る
↓
npm install
↓
dev serverを起動
↓
localhostのポートを確保
↓
ブラウザを開く
↓
確認する
↓
サーバーを止める
↓
一時ファイルを消す
```

これは以下の用途に対して過剰である。

- AIが生成したHTMLを一瞬だけ確認したい
- Markdownを軽くプレビューしたい
- Mermaid diagramを即表示したい
- コード断片をハイライト付きで見たい
- UIアイデアを使い捨てで試したい
- shell pipeline の出力をGUIとして見たい
- notebook的に単発のHTMLを出したい

本当に必要なのは：

```text
code → visible surface
```

であり、プロジェクトでもサーバーでもない。

---

# 4. ビジョン

TinyView を、Web UI版の `open` / `preview` コマンドにする。

例えば画像であれば：

```bash
open image.png
```

に近い感覚で、Web UIなら：

```bash
cat ui.html | tinyview
```

とできる。

将来的には、AI coding agent やエディタ、CLIツールが、TinyViewを一時UI表示の標準的な出力先として使える状態を目指す。

---

# 5. 最重要方針

## 5.1 起動速度が最優先

TinyView における最重要KPIは **起動速度** である。

このプロダクトの価値は、機能量ではなく：

```text
思いつき
↓
コマンド実行
↓
即視覚確認
```

までのレイテンシの小ささにある。

したがって、以下の順で優先する。

1. 起動速度
2. 初回描画速度
3. メモリ使用量
4. 操作レスポンス
5. 拡張性
6. 機能量

機能追加によって起動が遅くなる場合、その機能は core に入れず、template / plugin / optional feature として外出しする。

---

# 6. プロダクト原則

## 6.1 No Server

TinyView はローカルHTTPサーバーを立てない。

以下を禁止する。

- `localhost`
- port listen
- dev server
- background HTTP server
- hidden proxy server

---

## 6.2 No Port

TinyView はポートを占有しない。

`5173`、`3000`、`8000` などのポート番号をユーザーに意識させない。

---

## 6.3 No Generated Preview File

TinyView はプレビュー用HTMLを一時ファイルとして生成しない。

入力はメモリ上に読み込み、最終的に単一HTML文字列としてWebViewへ注入する。

ただし、以下は許容する。

- ユーザーが明示的に指定した入力ファイルの読み込み
- dotfile config の読み込み
- ユーザー定義 template ファイルの読み込み

重要なのは、TinyView が **プレビュー実行のための一時HTMLファイルを生成しない** ことである。

---

## 6.4 Ephemeral Runtime

実行時状態は一時的である。

ウィンドウを閉じると以下を破棄する。

- DOM state
- JavaScript state
- in-memory generated HTML
- runtime session
- selected template render context

TinyView はデフォルトでは永続的なアプリ状態を持たない。

---

## 6.5 Native WebView

Electron のようにChromiumを同梱しない。

OSネイティブのWebViewを利用する。

| Platform | Engine    |
| -------- | --------- |
| macOS    | WKWebView |
| Windows  | WebView2  |
| Linux    | WebKitGTK |

これにより、バイナリサイズ・起動速度・メモリ使用量を抑える。

---

## 6.6 CLI First

TinyView はCLIから使うことを第一に設計する。

GUIアプリとしての設定画面やタブUIは持たない。

---

# 7. 想定ユーザー

## 7.1 Primary User

### AI支援開発者

ChatGPT、Claude、Cursor、Cline、Copilot などで生成されたHTMLやUIを即確認したいユーザー。

代表例：

```bash
llm generate-ui | tinyview
```

---

## 7.2 Secondary User

### Frontend Engineer

UI断片やHTMLスニペットを素早く確認したい開発者。

### Tool Builder

CLIツールの出力先として、軽量なGUI surface を使いたい開発者。

### Educator

HTML / Markdown / Mermaid のサンプルを即座に表示したい教育用途。

---

# 8. 主要ユースケース

## 8.1 HTMLを直接表示

```bash
echo '<h1>Hello</h1>' | tinyview
```

## 8.2 Markdownプレビュー

```bash
cat README.md | tinyview -t markdown
```

## 8.3 Mermaidプレビュー

```bash
cat graph.mmd | tinyview -t mermaid
```

## 8.4 Code Highlightプレビュー

```bash
cat main.rs | tinyview -t code --param lang=rust
```

## 8.5 AI生成UIの即表示

```bash
llm "make a settings panel in html" | tinyview
```

## 8.6 独自ラッパー付きプレビュー

```bash
cat memo.md | tinyview -t my-markdown-layout
```

---

# 9. CLI仕様

## 9.1 基本

```bash
tinyview [source] [options]
```

## 9.2 stdin

```bash
cat app.html | tinyview
```

stdin が存在する場合、TinyView は stdin を優先的に入力として扱う。

## 9.3 ファイル入力

```bash
tinyview app.html
```

指定された path が存在する場合、そのファイルを読み込む。

## 9.4 インライン入力

```bash
tinyview --html '<h1>Hello</h1>'
```

## 9.5 Template指定

```bash
tinyview README.md --template markdown
```

short form:

```bash
tinyview README.md -t markdown
```

## 9.6 Template parameter

```bash
tinyview main.rs -t code --param lang=rust --param theme=github-dark
```

## 9.7 Window size

```bash
tinyview app.html --width 420 --height 800
```

## 9.8 Frameless

```bash
tinyview app.html --frameless
```

## 9.9 Transparent

```bash
tinyview app.html --transparent
```

## 9.10 Watch mode

```bash
tinyview README.md --watch
```

watch mode は file input 時のみ有効。

---

# 10. Template System

TinyView は単なるHTML viewerではなく、入力をWebView向けHTMLへ変換する **template runtime** を持つ。

基本構造：

```text
input
↓
template resolve
↓
template compile
↓
single HTML string
↓
WebView inject
```

最終的には必ず **単一HTML文字列** に変換してWebViewへ渡す。

---

# 11. Config / Dotfile仕様

## 11.1 Config Root

```text
~/.tinyview/
```

将来的には：

```text
$XDG_CONFIG_HOME/tinyview/
```

対応を検討する。

---

## 11.2 Directory Structure

```text
~/.tinyview/
├── config.toml
└── templates/
    ├── markdown/
    │   ├── template.toml
    │   ├── template.html
    │   ├── style.css
    │   └── marked.min.js
    │
    ├── mermaid/
    │   ├── template.toml
    │   ├── template.html
    │   └── mermaid.min.js
    │
    ├── code/
    │   ├── template.toml
    │   ├── template.html
    │   ├── style.css
    │   └── highlight.min.js
    │
    └── custom-layout/
        ├── template.toml
        └── template.html
```

---

# 12. Config File

## 12.1 Example

```toml
window_width = 1000
window_height = 760
default_template = "raw"

[extension]
md = "markdown"
markdown = "markdown"
mmd = "mermaid"
mermaid = "mermaid"
rs = "code"
ts = "code"
js = "code"
py = "code"

[templates.markdown.params]
theme = "github"

[templates.code.params]
theme = "github-dark"
line_numbers = true
```

---

# 13. Template Resolution

Template は以下の優先順位で決定する。

```text
explicit --template / -t
↓
extension mapping
↓
default_template
↓
raw
```

---

# 14. Template HTML仕様

Template HTML は最終的に単一HTMLへコンパイルされる。

## Reserved placeholders

| Placeholder                     | 内容                |
| ------------------------------- | ------------------- |
| `{{ input.json }}`              | 入力文字列          |
| `{{ input.html }}`              | HTML escape済み入力 |
| `{{ title }}`                   | title               |
| `{{ path }}`                    | 元path              |
| `{{ params.json }}`             | params              |
| `{{ inline_css("style.css") }}` | CSS inline          |
| `{{ inline_js("script.js") }}`  | JS inline           |
| `{{ asset_url("image.png") }}`  | data URL化          |

---

# 15. Built-in Templates

## MVP Built-ins

| Template  | 内容         |
| --------- | ------------ |
| `raw`     | HTMLそのまま |
| `text`    | plain text   |
| `minimal` | 最小shell    |

## Optional Built-ins

| Template   | 内容             |
| ---------- | ---------------- |
| `markdown` | Markdown preview |
| `mermaid`  | Mermaid preview  |
| `code`     | Syntax highlight |

重いtemplate assetは lazy load する。

---

# 16. Performance Requirements

## Raw path

```bash
echo '<h1>Hello</h1>' | tinyview
```

| Metric      | Target |
| ----------- | ------ |
| startup     | <150ms |
| first paint | <200ms |
| idle memory | <50MB  |
| binary size | <10MB  |

---

# 17. Startup Optimization Rules

## Coreに含めてよいもの

- stdin reader
- config loader
- template resolver
- native WebView launcher

## Coreに含めてはいけないもの

- embedded Chromium
- Node.js runtime
- npm integration
- TypeScript compiler
- React compiler
- heavy preload
- background daemon
- localhost server

---

# 18. Architecture

## 推奨スタック

```text
Rust
+
wry
+
tao
```

## Runtime Flow

```text
CLI start
↓
parse args
↓
read input
↓
load config
↓
resolve template
↓
compile single HTML
↓
create WebView
↓
inject HTML
↓
show window
↓
close
↓
exit
```

---

# 19. Security Model

## Default

TinyView は以下を提供しない。

- native bridge
- shell execution
- filesystem access from JS
- Node.js API

## Optional Permissions

```bash
tinyview app.html --allow-fetch
tinyview app.html --allow-clipboard
tinyview app.html --allow-storage
```

---

# 20. MVP Scope

## 必須

- stdin input
- file input
- inline HTML
- raw HTML rendering
- native WebView
- memory injection
- no server
- no port
- no generated preview file
- config.toml
- template runtime

## MVP外

- React build
- Vue build
- TypeScript transpile
- npm install
- HMR
- browser tabs
- GUI settings
- background daemon

---

# 21. 最終定義

TinyView は：

```text
CLIから渡された入力を、
templateで単一HTMLへ変換し、
native WebViewへメモリ注入して、
一時的に表示する超軽量runtime
```

である。

守るべき絶対条件：

```text
No server
No port
No generated preview file
Fast startup
Native WebView
Memory-first execution
Close means gone
Template composability
```

---

# 22. 最小の理想体験

```bash
echo '<h1>Hello</h1>' | tinyview
```

結果：

```text
一瞬でWebViewが開く
↓
Hello が表示される
↓
閉じる
↓
何も残らない
```

これが TinyView の中心体験である。
