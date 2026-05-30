# TinyView PRD

**Version:** v0.2
**Status:** Draft
**Product Type:** CLI-first transient WebView runtime
**Core Value:** サーバーなし、ポートなし、物理プレビュー生成なしで、CLIから即座にWeb UIを表示する

> v0.2 での主要変更: Template System を「placeholder runtime + multi-file directory」から「単一HTML + `window.__TINYVIEW__` 注入」に簡素化（§10 / §11.2 / §14）。

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

## 6.7 Non-blocking CLI

TinyView は launch 後 **ただちに shell プロンプトへ制御を返す**。WebView ウィンドウは親プロセスから切り離されて独立に動作する。

```bash
echo '<h1>x</h1>' | tinyview
$ █  # ← 即座に次コマンド入力可
```

これを破ると CLI tool としての composability が著しく損なわれる（`open file.png` / `xdg-open` と同じ UX を期待される）。

実装は **detach-by-default**:

- Unix: `fork()` → `setsid()` → 親は即 `exit 0`、子が WebView を保持
- Windows: 自身を `CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS` で再 spawn し、親プロセスは即終了

stdin / file / config / template 解決・HTML 合成は **すべて親プロセスで完了させる**。検証エラーは親が non-zero exit で返すため shell から検出可能。fork 後のエラーは WebView 内に表示するか、silent exit する。

`--foreground` フラグで明示的に前景維持も可能（`--watch` で Ctrl+C kill したい / CI / debug 用途）。

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

watch mode は **file input 時のみ有効**（stdin / `--html` と併用するとエラー、exit code 2）。

**`--watch` は `--foreground` を暗黙的に強制する**。detach すると親→子へ source path / template / params を渡す protocol が必要で複雑化するため。watch はインタラクティブ用途（ユーザーが編集しながら更新を見る）であり foreground で十分。Ctrl+C で kill 可能。

ファイル更新検出は `notify` + `notify-debouncer-mini` で行い、**100ms trailing debounce** を適用する（VSCode / Vim / IntelliJ の atomic save パターンに対応）。検出時は入力を再読込し、同じ template で再 render（template は再 resolve しない）して `WebView::load_html()` で content swap する。WebView 自体は破棄せず維持するため、スクロール位置・focus は reset される（PRD §9.10 の ephemeral 思想と整合）。

CSP `<meta>` 注入は reload 時にも適用される（template HTML が CSP を持たない前提のため、毎回 runtime が注入する責務を持つ）。

実装上の注意: macOS FSEvents は親ディレクトリ単位のイベントのみを返すため、reader 側で `event.paths` と target path の一致確認が必須。さらに atomic save (`write tempfile + rename`) で inode が変わるパターンに対応するため、**target ファイル自体ではなく親ディレクトリを `RecursiveMode::NonRecursive` で watch する** 実装にすること。

## 9.11 Foreground mode

```bash
tinyview app.html --foreground
```

デフォルトでは launch 後 shell に制御を返す（§6.7）。`--foreground` を指定すると detach せず前景に留まる。Ctrl+C で kill したい CI / debug 用途で使う。

`--watch` は §9.10 のとおり `--foreground` を暗黙的に強制するため、`tinyview README.md --watch` は明示的な `--foreground` 指定不要。

---

# 10. Template System

TinyView の template は **単一HTMLファイル** である。Runtime は独自の template 言語を持たず、template に対して以下を1箇所だけ注入する：

```text
window.__TINYVIEW__ = { input, params, title, path }
```

これにより、Rust 側の処理は実質「1回の文字列置換」に縮退し、複雑な template engine / placeholder 文法 / asset inlining / path 解決ロジックを持たない。

基本構造：

```text
input
↓
template resolve
↓
inject window.__TINYVIEW__ (single marker substitution)
↓
single HTML string
↓
WebView inject
```

`raw` mode では substitution も発生せず、入力 HTML をそのまま WebView へ渡す（最速パス）。

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
    ├── markdown.html
    ├── mermaid.html
    ├── code.html
    └── custom-layout.html
```

各 template は **自己完結した単一HTMLファイル**である。CSS / JS / library (例: `marked.js`) は `<style>` / `<script>` として template 内に inline する。外部ファイル参照 (`<link href>` / `<script src>`) は no server / no port 原則により解決できないため禁止。

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

## 13.1 raw mode の最速パス

`raw` が選ばれた場合、TinyView は以下を **すべて skip** して `WebView::load_html(&input)` に直行する:

- `~/.tinyview/config.toml` の読み込み
- `--param` の評価（**raw mode では `--param` は無視される**）
- `window.__TINYVIEW__` JSON literal の生成
- template ファイルの load と marker substitution

これは §16 起動目標 (<150ms) を満たすための最短経路である。`--param` を反映させたい場合は `text` / `minimal` / カスタム template を明示すること。

---

# 14. Template Contract

TinyView は **template engine / placeholder 文法を持たない**。Template HTML は1箇所のマーカーを介して runtime からデータを受け取る。

## 14.1 Injection Marker

Template はどこか1箇所（慣習的に `<head>` 内）に以下を含む：

```html
<script>
  window.__TINYVIEW__ = /*__TINYVIEW__*/ null /*__TINYVIEW__*/;
</script>
```

Runtime はこのマーカー（`/*__TINYVIEW__*/ null /*__TINYVIEW__*/`）を1回の文字列置換で JSON literal に差し替える。マーカーが存在しない場合は警告のみで続行する（raw 用途）。

## 14.2 Injected Object

```ts
window.__TINYVIEW__ = {
  input: string, // 入力本体（stdin / file / --html のいずれか）
  params: Record<string, string>, // --param k=v および config の [templates.X.params]
  title: string,
  path: string | null, // file 入力時のみ。stdin / inline は null
};
```

## 14.3 Template の責務

- **HTML escape は template 側 JS の責任**。`element.textContent = window.__TINYVIEW__.input` の形で安全に注入する
- **CSS / JS / library は template 内に inline**。`<style>...</style>` / `<script>...</script>` に直接埋め込む
- **外部リソース禁止**。`<link rel="stylesheet" href="...">` / `<script src="...">` は server を持たないため解決できない
- **描画ロジックを template 側に置く**。Rust 側は markdown / mermaid 等のパースを行わない

---

# 15. Built-in Templates

## MVP Built-ins

| Template  | 内容         | 実装形態                                                                                                                                          |
| --------- | ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| `raw`     | HTMLそのまま | **テンプレートファイル無し**。resolver が `raw` を返したら load も substitution も skip し、入力をそのまま `WebView::load_html()` へ渡す（§13.1） |
| `text`    | plain text   | 単一HTMLファイルとして `include_str!("text.html")` でバイナリ同梱（~1.2KB）                                                                       |
| `minimal` | 最小shell    | 単一HTMLファイルとして `include_str!("minimal.html")` でバイナリ同梱（~1.2KB）                                                                    |

- `text`: `<pre>` + `pre-wrap` + `ui-monospace` font stack。`textContent` で安全注入（HTML として解釈しない）
- `minimal`: `<main>` 中央寄せ + max-width 760px。`innerHTML` で `<html><head><body>` を持たない HTML fragment を流す（AI 生成 HTML の主要ユースケース）。**信頼境界は呼び出し側にある**（`<script>` がそのまま実行される）
- 両 template ともに `color-scheme: light dark` + system color で OS ダークモード追従。Web font は使わない（no server / 起動遅延 / ephemeral すべてに違反）

## Optional Built-ins

| Template   | 内容             |
| ---------- | ---------------- |
| `markdown` | Markdown preview |
| `mermaid`  | Mermaid preview  |
| `code`     | Syntax highlight |

重いtemplate assetは lazy load する。これらは `~/.tinyview/templates/<name>.html` に必要な library (`marked.min.js` 等) を `<script>` で inline 同梱する形で配布する。

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
[parent process]
CLI start
↓
parse args (clap, feature絞り)
↓
read input (stdin > file > --html)            ← stdin はここで全消費
↓
load config (lazy: rawパスでは呼ばない)
↓
resolve template (explicit > ext > default > raw)
↓
substitute __TINYVIEW__ marker (rawではスキップ)
↓
[validate complete — ここまでで失敗したら non-zero exit]
↓
fork & detach (--foreground 指定時はスキップ)  ← §6.7
        │
        ├──→ [parent] exit 0 immediately
        │
        ▼
[child process]
create WebView (wry + tao)
↓
inject HTML (load_html, no temp file)
↓
[--watch のみ] spawn notify watcher thread
↓
event loop
↓
close
↓
exit (全state破棄)
```

## 主要依存

| 領域              | 採用                                                                                                                                                | 備考                                                                                                                |
| ----------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| WebView / window  | `wry` + `tao`                                                                                                                                       | OS ネイティブ WebView 直叩き                                                                                        |
| CLI parse         | `clap`                                                                                                                                              | `default-features = false`, `derive` + `help` + `usage` + `error-context` のみ                                      |
| Config / params   | `serde` + `toml`                                                                                                                                    | config が必要なときだけロード                                                                                       |
| JSON 注入         | `serde_json`                                                                                                                                        | `window.__TINYVIEW__` の literal 生成                                                                               |
| Watch (`--watch`) | `notify` + `notify-debouncer-mini`（常時リンク、Cargo feature `watch` default=on）                                                                  | 起動時 init しないため raw path への runtime cost ~0。バイナリ +~200KB。組み込み版用に feature off の逃げ道だけ残す |
| Template engine   | **なし**                                                                                                                                            | `str::replace` 1回で完結                                                                                            |
| Detach (Unix)     | `libc::fork` + `libc::setsid`                                                                                                                       | runtime cost ~1-3ms。`--foreground` でスキップ可                                                                    |
| Detach (Windows)  | `std::os::windows::process::CommandExt` で `DETACHED_PROCESS \| CREATE_NEW_PROCESS_GROUP` 指定して self-respawn、合成済み HTML を子の stdin へ pipe | 同上                                                                                                                |

---

# 19. Security Model

## 19.1 デフォルト拒否ポリシー

TinyView は以下をデフォルトで提供しない。

- native bridge / JS ↔ Rust IPC
- shell execution
- filesystem access from JS
- Node.js API
- DevTools（debug build を除く）
- 外部 HTTP/HTTPS リクエスト（`fetch` / XHR / WebSocket）
- Clipboard API（macOS は OS 制約により完全拒否不可、§19.3 参照）
- 永続ストレージ（localStorage / IndexedDB / Cookie / HTTP cache）
- top-level の外部 URL 遷移

## 19.2 Optional Permissions

```bash
tinyview app.html --allow-fetch
tinyview app.html --allow-clipboard
tinyview app.html --allow-storage
```

| フラグ              | runtime 動作                                                                                   |
| ------------------- | ---------------------------------------------------------------------------------------------- |
| `--allow-fetch`     | 注入 CSP の `connect-src` を `'none'` から `https: http: ws: wss:` に緩和                      |
| `--allow-clipboard` | wry `with_clipboard(true)`（macOS では既にOS側で許可されており差分なし）                       |
| `--allow-storage`   | wry `with_incognito(false)` + `WebContext` を永続パスで構成。プロセス終了後も DataStore が残る |

## 19.3 Ephemeral 実装上の必須要件

PRD §6.4「閉じたら全破棄」を実装で守るため、以下を **runtime のデフォルト** として組み込む。`--allow-*` 指定で対応スライスのみ緩和される。

- **`WebViewBuilder::with_incognito(true)` をデフォルト ON**
  - 未指定で起動すると wry は `%LOCALAPPDATA%\<exe>\EBWebView`（Windows）/ `~/Library/WebKit/...`（macOS）等にデータをディスク永続させる → ephemeral 違反
  - **WebView2 では runtime 101+ 必須**。古い環境では `build()` が失敗する可能性があるため、検出時は一時 data directory を割り当て、プロセス終了時 (`Drop` / `atexit` 相当) に削除する fallback 経路を用意する
- **`WebViewBuilder::with_devtools(false)` をデフォルト ON**（debug build のみ true）
- **`WebViewBuilder::with_clipboard(false)` をデフォルト ON**
  - Linux / Windows では実効
  - **macOS WKWebView は OS 標準で常時 ON のため API レベルで拒否できない**。`navigator.clipboard` を `with_initialization_script` で `undefined` にする補強を入れるが、ネイティブショートカット (Cmd+C/V) は塞げない。この事実は仕様上の制約として受容する
- **`with_navigation_handler` で top-level 外部遷移を拒否**（`about:` / `data:` のみ許可）
- **CSP `<meta http-equiv>` を runtime が HTML へ注入する**
  - デフォルト: `default-src 'self' 'unsafe-inline' data: blob:; connect-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none';`
  - `'unsafe-inline'` を許容するのは Template Contract (§14.3) が CSS/JS の inline を要求するため。代替に nonce 方式があるが起動コスト増のため不採用

## 19.4 Template と CSP の境界

CSP は runtime が排他的に管理する。**Template HTML 側に `<meta http-equiv="Content-Security-Policy">` を書いてはならない**。両方が存在すると最も制限的な値が勝ち、template が想定外に壊れる。

## 19.5 raw mode と Security

`raw` mode (§13.1) でも §19.3 の WebView builder デフォルト (incognito / devtools / clipboard / navigation_handler) は適用される。ただし CSP `<meta>` 注入は起動最速優先のため **skip する** — raw mode の利用者は信頼できる入力を流す責務を負う。`--allow-*` フラグが指定された場合のみ CSP 注入が走る。

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
- single-file template + `window.__TINYVIEW__` 注入

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
