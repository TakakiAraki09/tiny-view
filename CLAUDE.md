# CLAUDE.md

このリポジトリで作業する際の指針。詳細仕様は `docs/PRD.md` を参照。

## プロダクト概要

**TinyView** は CLI から即座に Web UI を表示する一時実行型プレビューランタイム。

```bash
echo '<h1>Hello</h1>' | tinyview
```

サーバー・ポート・一時HTMLファイル生成なしで、ネイティブ WebView 上に描画し、閉じたら状態は全て破棄される。位置付けは「軽量ブラウザ」ではなく `ephemeral rendering primitive`。

## 絶対条件（破ってはならない原則）

実装提案・コードレビュー時は常にこれらに反していないかチェックする。

- **No Server**: localhost / port listen / dev server / background HTTP server / hidden proxy のいずれも禁止
- **No Port**: ユーザーにポート番号を意識させない
- **No Generated Preview File**: プレビュー実行のための一時HTMLを書き出さない（入力はメモリ上で単一HTML文字列に合成し WebView へ注入）
  - ユーザー指定の入力ファイル / dotfile config / template ファイルの読み込みは可
- **Ephemeral Runtime**: ウィンドウを閉じたら DOM / JS state / 生成HTML / session を全破棄。デフォルトで永続状態を持たない
- **Native WebView**: Chromium 同梱禁止。macOS=WKWebView / Windows=WebView2 / Linux=WebKitGTK を使う
- **CLI First**: GUI設定画面やタブUIを持たない
- **Non-blocking CLI**: launch 後ただちに shell に制御を返す（`open file.png` と同じ挙動）。`--foreground` 明示時のみ前景維持。stdin/file/template の検証は親プロセスで済ませてから fork/detach する

## 最重要 KPI: 起動速度

優先順位は固定:

1. 起動速度
2. 初回描画速度
3. メモリ使用量
4. 操作レスポンス
5. 拡張性
6. 機能量

raw path（`echo '<h1>x</h1>' | tinyview`）のターゲット:

| Metric      | Target |
| ----------- | ------ |
| startup     | <150ms |
| first paint | <200ms |
| idle memory | <50MB  |
| binary size | <10MB  |

**起動を遅くする機能は core に入れない。** template / plugin / optional に外出しする。

## Core に入れて良いもの / いけないもの

**Core OK:** stdin reader / config loader / template resolver / native WebView launcher

**Core NG:** embedded Chromium / Node.js runtime / npm 統合 / TypeScript compiler / React compiler / heavy preload / background daemon / localhost server

## 推奨スタック

```
Rust + wry + tao
+ clap (default-features = false)
+ serde / toml / serde_json
```

別言語・別フレームワークを提案する場合は、起動速度ターゲットとバイナリサイズ <10MB をどう満たすか必ず説明する。Template engine (minijinja / tera / handlebars 等) は採用しない — 仕様上不要。

## 入力解決の優先順位

1. stdin（存在すれば最優先）
2. ファイル path 引数
3. `--html` インライン

## Template 解決の優先順位

1. 明示指定 `--template` / `-t`
2. 拡張子マッピング（config の `[extension]`）
3. `default_template`
4. `raw`

最終的に必ず**単一HTML文字列**にコンパイルしてから WebView に渡す。

## Config

- Root 解決順（存在する最初を採用）: `$XDG_CONFIG_HOME/tinyview/` → `~/.config/tinyview/` → `~/.tinyview/`（後方互換 fallback）。詳細は `docs/PRD.md` §11.1
- `config.toml` + `templates/<name>.html`（自己完結した単一HTML）の構成
- 詳細は `docs/PRD.md` §11–§15

## Template Contract

- TinyView は template engine を持たない。**`str::replace` 1回**だけ
- Template の `<head>` に `<script>window.__TINYVIEW__ = /*__TINYVIEW__*/ null /*__TINYVIEW__*/;</script>` を置き、runtime が JSON literal で置換する
- 注入される値: `{ input, params, title, path }`
- HTML escape / library 同梱 / 描画ロジックはすべて template 側 JS の責任
- 外部リソース参照 (`<link href>` / `<script src>`) は no server 原則により禁止

## MVP スコープ

**含む:** stdin / file / inline HTML 入力、raw 描画、native WebView、memory injection、config.toml、template runtime

**含まない:** React/Vue build、TS transpile、npm install、HMR、tabs、GUI設定、background daemon

## セキュリティデフォルト

JS から native bridge / shell exec / filesystem / Node API は提供しない。必要な場合のみ `--allow-fetch` / `--allow-clipboard` / `--allow-storage` で明示許可。fetch は template/HTML の `<head>` に `<meta name="tinyview-allow" content="fetch">` を置くことでも許可できる（`--allow-fetch` と OR。PRD §19.2.1）。

## 作業時のチェックリスト

新機能・依存追加・アーキテクチャ変更を提案する前に:

- [ ] 起動速度ターゲット（<150ms）を悪化させないか
- [ ] バイナリサイズ <10MB を壊さないか
- [ ] No Server / No Port / No Generated Preview File に反していないか
- [ ] core ではなく template / plugin に外出しできないか
- [ ] 永続状態を持ち込んでいないか（ephemeral 原則）

## リリースとコミット規約

リリースは **release-plz**（`.github/workflows/release.yml`）が自動化する。AI agent / 貢献者は必ずこのフローに従う。

### コミットは Conventional Commits 必須

release-plz は**コミットメッセージから次バージョンを自動算出**する。main に入るコミットはバージョンに影響するため Conventional Commits に従うこと:

| prefix | bump | 例（1.0.4 起点） |
| ------ | ---- | ---------------- |
| `fix:` | patch | 1.0.**5** |
| `feat:` | minor | 1.**1**.0 |
| `feat!:` / `fix!:` / フッター `BREAKING CHANGE:` | major | **2**.0.0 |
| 上記以外（`chore:` `docs:` `ci:` `refactor:` `test:` / 非 conventional） | patch（デフォルト） | 1.0.**5** |

- 複数コミットがあれば**最も強い bump が勝つ**。
- release-plz のデフォルトでは `feat` / breaking 以外はすべて patch 扱いなので、`chore:` だけでも Release PR は patch bump を提案する（CHANGELOG には全コミットが載る）。`chore`/`docs` でリリースを切りたくない場合は `release-plz.toml` の設定が要る。
- 加えて release-plz は `cargo-semver-checks` で実 API の破壊も検査し、コミットが patch でも API 破壊なら major に引き上げる。

### バージョン / タグ / Cargo.lock を手で触らない

- `Cargo.toml` の `version`、`Cargo.lock`、`git tag` は **release-plz が所有する**。手動で bump・タグ付けをしない（過去にこの手作業で Cargo.lock がドリフトし `cargo publish` が壊れた）。
- バージョンを上げたいときは適切な Conventional Commit を積むだけでよい。

### リリースフロー

1. main に push → release-plz が単一の **Release PR**（version bump + CHANGELOG + Cargo.lock 同期）を自動で開く / 更新する
2. リリースしたくなったら **Release PR をマージ**する → そのマージ（main への push）で release-plz が **自動的に `cargo publish` + tag + GitHub Release** まで実行する

**Release PR のマージ自体がリリースゲート**。マージは人間の明示操作なので、不可逆な crates.io publish（再公開不可）のゲートを兼ねる。手動 `workflow_dispatch` は廃止した（マージと二重のゲートで冗長だったため）。bump と Cargo.lock 同期は Release PR 上でレビューされてからマージされるので、publish 前に必ず人間の目を通る。

## ドキュメント

- `docs/PRD.md` — Single source of truth。仕様の解釈に迷ったら必ず参照
