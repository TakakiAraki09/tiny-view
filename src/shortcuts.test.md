# shortcuts.rs テスト

ネイティブメニューバーの「構成（レイアウト）」を検証する。`shortcuts.rs` は
muda / AppKit に依存しない純粋データで、メニューに何が・どの順で・どの
ショートカットで並ぶかの唯一の真実。macOS のメニュー構築（`menu.rs`）はこの
`MENU_LAYOUT` を走査するだけなので、ここを固めればメニュー内容のリグレッション
を防げる。GUI / NSApp 不要で全項目を検証できる。

> macOS 限定でコンパイルされる（メニュー機能自体が macOS のみ）。テストは
> macOS CI ランナーで実行される。

## 検証事項

|検証事項|内容|備考|
|---|---|---|
|必須ショートカットが揃う|Quit(⌘Q) / Enter Full Screen(⌃⌘F) / Close(⌘W) / Minimize(⌘M) / Copy(⌘C) / Paste(⌘V) がレイアウトに存在し、期待アクセラレータを持つ。|ユーザー要望の中核|
|先頭はアプリメニュー|macOS の慣習どおり 1 番目のセクションがアプリメニュー（"TinyView"）で、Quit を含む。|first_section|
|セパレータが宙に浮かない|各セクションでセパレータが先頭・末尾に来ない／連続しない（二重線にならない）。|separator|
|アクセラレータは一意|操作可能な項目どうしでアクセラレータが重複しない（ショートカットの食い合いを防ぐ）。|uniqueness|
|セクションは題と中身を持つ|全セクションがタイトル非空・項目 1 個以上。|structural|
|セパレータはアクセラレータ無し|`Separator` は操作不能なので `accelerator()` が `None`。|separator / accelerator|
|spec と variant 集合の整合|操作可能な項目は必ずアクセラレータを持つ（例外は標準で無しの About / Show All）。新しい項目を追加してアクセラレータ判断を忘れると落ちる。|accelerator / リグレッションガード|

## メニュー構成（実装の一覧）

`MENU_LAYOUT` が定義する内容。muda の predefined item は AppKit 標準セレクタ
（`terminate:` / `toggleFullScreen:` / `copy:` …）にマップされ、WKWebView が
フォーカスを持っていてもメニュー経由で機能する。

|メニュー|項目（ショートカット）|
|---|---|
|TinyView（アプリ）|About / Hide(⌘H) / Hide Others(⌥⌘H) / Show All / Quit(⌘Q)|
|Edit|Cut(⌘X) / Copy(⌘C) / Paste(⌘V) / Select All(⌘A)|
|View|Enter Full Screen(⌃⌘F)|
|Window|Minimize(⌘M) / Close(⌘W)|

## 備考

- アクセラレータは muda の predefined item が付与する macOS デフォルト値。
  `shortcuts.rs` の `accelerator()` はそれを渡すためではなく、**期待値（spec）**
  として保持し、テストで突き合わせる。実際の画面表示との一致は GUI 操作確認の
  範囲（手元での目視）。
