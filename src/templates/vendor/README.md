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
| `hljs-theme.css` | highlight.js github / github-dark themes (combined) | 11.11.1 | CC0-1.0 | styles/github.min.css + styles/github-dark.min.css |
| `mermaid.min.js` | mermaid | 11.4.1 | MIT | https://cdn.jsdelivr.net/npm/mermaid@11.4.1/dist/mermaid.min.js |

## Updating

Re-download the exact version from the source URL above and replace the file. Verify:

1. The minified JS contains no `</script>` / `</style>` substring (would break inline embedding).
2. The library still exposes its expected global (`marked`, `hljs`, `globalThis.mermaid`).
3. It needs no `eval` / `new Function` (CSP has no `'unsafe-eval'`).
4. Binary size stays `< 10MB` (`cargo build --release`).
