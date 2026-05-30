# template.rs テスト

テンプレートの「解決」と「描画（マーカー置換）」を検証する。
解決の優先順位は PRD §13 に準拠する。

## 検証事項

|検証事項|内容|備考|
|---|---|---|
|explicit がすべてに勝つ|`--template text` を渡せば、拡張子マッピングや default があっても `Text`。|resolve|
|explicit が無ければ拡張子で決まる|`notes.md` + `[extension] md = minimal` → `Minimal`。|resolve|
|拡張子もヒットしなければ default|未知拡張子 `a.unknown` なら `default_template` を採用。|resolve|
|どれも無ければ raw にフォールバック|引数すべて None → `Raw`。|resolve|
|未知の名前は user テンプレ扱い|`custom-layout` → `User(custom-layout.html)`。caller が config root に join する前提。|resolve|
|オプション組み込み名を解決できる|`markdown` / `mermaid` / `code` がそれぞれ対応する `TemplateRef` になる。|resolve|
|マーカーは1回だけ置換|`/*__TINYVIEW__*/ null /*__TINYVIEW__*/` が消え、`"input"` / `"title"` が JSON literal に入る。path 無しは `"path":null`。|render|
|minimal は params と path も載る|`theme=github` と `path` が JSON に出る。|render|
|マーカーが無ければ原文のまま|user テンプレにマーカーが無い場合、HTML を変えずに返す（stderr に warning）。|render|
|markdown は marked + highlight.js をインライン|lib プレースホルダが消え `marked` / `hljs` が埋め込まれる。`<script src=` / `<link ` は出ない。|render / No Server 原則|
|code は highlight.js をインライン|`lang=rust` が JSON に載り、hljs が埋め込まれる。|render|
|mermaid は mermaid.js をインライン|外部 src 参照なし。`A-->B` の `>` はエスケープされるが、JSON round-trip で `input` が元の文字列に復元される。|render|
|raw は render を呼ぶと panic|raw は描画をバイパスする契約なので、呼び出し自体が論理エラー。|render / PRD §13.1|
|`</script>` を含む入力を無害化|注入リテラルに生の `</script>` / `<` が残らず（`<` 化）、JSON round-trip で `input` が `before</script>after` に一致。inline `<script>` が途中で閉じない。|build_literal / issue #29|
|JS 行終端子 U+2028 / U+2029 を無害化|生の U+2028 / U+2029 が残らず `\uXXXX` 化され、JS オブジェクトリテラル評価時の構文エラーを防止。`input` はロスなく復元。|build_literal / issue #29|
