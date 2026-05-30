# menu.rs テスト

`shortcuts::MENU_LAYOUT` の各 `MenuItem` を muda の predefined item に変換する
`predefined()` のマッピングを検証する。`MENU_LAYOUT` の中身（並び・ショート
カット）は `shortcuts.rs` 側でテスト済みなので、ここは「データ → muda 項目への
写像が正しいか」に絞る。

> macOS 限定。`install()` はメニューを NSApp に設置する GUI 起動経路で
> メインスレッドを要するため単体テストせず、スモーク起動で確認する。
> 一方 `predefined()` 単体は muda が NSMenuItem 生成を append 時まで遅延する
> ため、cargo test のワーカースレッド（非メインスレッド）でも安全に構築できる。

## 検証事項

|検証事項|内容|備考|
|---|---|---|
|意図した action に写像される|各 `MenuItem` を変換した predefined item の既定ラベル（`text()`）に期待文字列が含まれる（例: `Quit`→"Quit"、`Fullscreen`→"Full Screen"、`CloseWindow`→"Close"）。`Quit => copy(None)` のような取り違えを検出。|exhaustive match では防げない論理ミスのガード|
|レイアウト全項目が panic せず構築できる|`MENU_LAYOUT` の全項目（`Separator` と `About` メタデータ経路を含む）を変換しても落ちない。|スモーク|
|セパレータはラベル無し|`Separator` は区切り線なので `text()` が空。|separator|

## 備考

- ラベルは muda の predefined テーブルで `&` ニーモニック付き（例 `"&Quit"`）だが
  `text()` は除去済みを返すため、期待文字列は `&` 無しで突き合わせる。
- `About` / `Hide` / `Quit` はラベルにアプリ名を含む（`NSRunningApplication`
  から取得）。テスト環境ではアプリ名が空になり得るが、`"Quit"` 等の部分文字列は
  常に含まれるため部分一致で検証している。
- predefined item は AppKit 標準セレクタにマップされるため、ショートカットの
  実挙動（⌘Q で終了する等）は GUI 操作確認の範囲。
