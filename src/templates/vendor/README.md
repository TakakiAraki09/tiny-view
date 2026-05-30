# Vendored libraries

These files are third-party JavaScript/CSS embedded at compile time (`include_str!`)
into TinyView's optional built-in templates (`markdown` / `mermaid` / `code`).

They are inlined into a **single self-contained HTML string** before the document is
handed to the WebView. Nothing here is fetched at runtime — this preserves the
**No Server** principle (CSP `connect-src 'none'`) and keeps templates self-contained
per PRD §14 Template Contract.

| File | Library | Version | License | Source |
|------|---------|---------|---------|--------|
| `marked.min.js` | marked | 15.0.7 | MIT | https://cdn.jsdelivr.net/npm/marked@15.0.7/marked.min.js |
| `highlight.min.js` | highlight.js (core + common languages) | 11.11.1 | BSD-3-Clause | https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.11.1/highlight.min.js |
| `hljs-theme.css` | highlight.js github / github-dark themes (combined, light default + dark via `prefers-color-scheme`) | 11.11.1 | CC0-1.0 | styles/github.min.css + styles/github-dark.min.css |
| `mermaid.min.js` | mermaid | 11.4.1 | MIT | https://cdn.jsdelivr.net/npm/mermaid@11.4.1/dist/mermaid.min.js |

## Updating

Re-download the exact version from the source URL above and replace the file. Verify:

1. The minified JS contains no `</script>` / `</style>` substring (would break inline embedding).
2. It contains neither the data marker `/*__TINYVIEW__*/ null /*__TINYVIEW__*/` nor any
   `/*__TINYVIEW_<LIB>__*/` placeholder string (would corrupt the compile-time composition).
3. The library still exposes its expected global (`marked`, `hljs`, `globalThis.mermaid`).
4. It still renders under the runtime CSP — `connect-src 'none'` and **no `'unsafe-eval'`**.
   The bundles may *contain* `Function(` in dead/guarded paths; what matters is that the
   exercised render path does not trigger an eval CSP violation. Verified for basic cases via
   the `write_builtin_render_fixtures` ignored test in `src/webview.rs` (load the emitted HTML
   in a CSP-enforcing browser and confirm it renders with no console CSP error).
5. Binary size stays `< 10MB` (`cargo build --release`).
