# TinyView

> Ephemeral CLI WebView runtime — render Web UI from a pipe, no server, no port, no temp file.

```bash
echo '<h1>Hello</h1>' | tinyview
```

A native WebView window opens, the HTML paints, you close it, nothing persists. TinyView is the
Web UI counterpart of `open file.png` — a one-shot rendering primitive, not a browser.

---

## What & why

Confirming a small piece of HTML / Markdown / Mermaid / UI snippet usually means:
`scaffold → npm install → dev server → localhost:5173 → open browser → kill server → clean up`.
That pipeline is overkill when you only want to *see* the output once.

TinyView collapses it to:

```text
input  →  immediate render  →  close, gone
```

It targets AI-assisted workflows (`llm "make a settings panel" | tinyview`), UI snippet review,
Markdown / Mermaid preview, and shell pipelines that want a GUI surface as their final stage.

See [`docs/PRD.md`](docs/PRD.md) for the full product definition.

---

## Highlights

- **No server.** No localhost, no port listen, no background daemon.
- **No temp file.** Input is composed in memory and injected directly into the WebView.
- **Detach by default.** Shell prompt returns immediately (`open`-style UX). `--foreground` opts out.
- **Native WebView.** macOS WKWebView / Windows WebView2 / Linux WebKitGTK. No Chromium.
- **Ephemeral.** Close the window and DOM, JS state, in-memory HTML, and session are gone.
- **Small.** Release binary is ~1 MB; no Node, no bundler, no runtime preload.
- **Template system without an engine.** A single HTML file plus one `window.__TINYVIEW__` JSON
  injection — no placeholder grammar, no asset resolver.

---

## Performance

Measured on the current MVP (raw path: `echo '<h1>x</h1>' | tinyview`).

| Metric                | Target  | Observed     |
| --------------------- | ------- | ------------ |
| Parent exit (detach)  | —       | ~10–30 ms    |
| Cold startup          | <150 ms | within target |
| First paint           | <200 ms | within target |
| Idle memory           | <50 MB  | within target |
| Release binary size   | <10 MB  | ~1 MB         |

`raw` mode (no `--template`, no `--param`, no file path) skips config load, template load, and
marker substitution entirely — the WebView receives the input HTML as-is.

---

## Install

There is no published binary or `cargo install` target yet. Build from source:

```bash
git clone https://github.com/TakakiAraki09/tiny-view.git
cd tiny-view
cargo build --release
# binary is at ./target/release/tinyview
```

Add it to your `PATH` (example):

```bash
ln -s "$PWD/target/release/tinyview" /usr/local/bin/tinyview
```

Requires Rust 1.75+. On Linux you also need WebKitGTK development headers
(`libwebkit2gtk-4.1-dev` on Debian/Ubuntu).

---

## Usage

### Pipe HTML from stdin

```bash
echo '<button onclick="alert(1)">OK</button>' | tinyview
```

### Open a file

```bash
tinyview app.html
```

### Inline HTML

```bash
tinyview --html '<h1>Hello</h1>'
```

### Plain text (escaped, monospaced)

```bash
cat notes.txt | tinyview -t text
```

### Minimal shell for AI-generated fragments

```bash
llm "make a settings panel in html" | tinyview -t minimal
```

`minimal` wraps an HTML fragment in a centered, max-width 760px shell that follows the OS dark mode.

### Render Markdown

```bash
tinyview README.md -t markdown
```

`markdown` parses the input with [marked](https://marked.js.org/) and highlights fenced code blocks
with [highlight.js](https://highlightjs.org/) — both inlined into the document, nothing fetched.

### Render a Mermaid diagram

```bash
tinyview graph.mmd -t mermaid
# theme follows OS dark mode; override with --param theme=forest|neutral|dark|default
```

### Syntax-highlight source code

```bash
tinyview src/main.rs -t code --param lang=rust
# omit lang to auto-detect
```

`markdown` / `mermaid` / `code` are **optional built-ins**: their libraries are embedded in the
binary at compile time and inlined at render time, so they keep the No-Server / self-contained
contract and never touch the `raw` fast path. See [`src/templates/vendor/`](src/templates/vendor/)
for library provenance.

### Watch a file and reload on save

```bash
tinyview README.md --watch -t minimal
```

`--watch` is file-input only and implies `--foreground` (so `Ctrl+C` kills it). Atomic saves from
VS Code / Vim / IntelliJ are handled via a 100 ms trailing debounce on the parent directory.

### Stay in the foreground (CI / debug)

```bash
tinyview app.html --foreground
```

---

## CLI reference

| Flag                  | Description                                                          |
| --------------------- | -------------------------------------------------------------------- |
| `<source>`            | Path to an HTML file. Overridden by stdin if a pipe has data.        |
| `--html <string>`     | Inline HTML literal.                                                 |
| `-t, --template <n>`  | Template: `raw`/`text`/`minimal`/`markdown`/`mermaid`/`code`/user.    |
| `--param key=value`   | Template parameter, repeatable. Ignored in `raw` mode.               |
| `--width <px>`        | Window width (default 1000, or `window_width` in config).            |
| `--height <px>`       | Window height (default 760, or `window_height` in config).           |
| `--frameless`         | Remove window decorations (title bar / chrome).                      |
| `--transparent`       | Transparent window background. Combine with `rgba(_,_,_,<1)` CSS.    |
| `--watch`             | Reload on file change. File input only. Implies `--foreground`.      |
| `--foreground`        | Skip detach; stay attached to the shell.                             |
| `--allow-fetch`       | Loosen CSP `connect-src` to allow outbound `fetch` / XHR / WS.       |
| `--allow-clipboard`   | Enable clipboard API in the WebView. See *Permissions* below.        |
| `--allow-storage`     | Persist WebView storage (disables incognito mode).                   |
| `-h, --help`          | Show usage.                                                          |
| `-V, --version`       | Show version.                                                        |

Input resolution priority: **stdin (when it has data) > file path > `--html`**.

---

## Configuration

Config lives in the TinyView config root. The root is resolved in this order, picking the first
directory that exists:

1. `$XDG_CONFIG_HOME/tinyview/` (when `XDG_CONFIG_HOME` is set)
2. `$HOME/.config/tinyview/` (XDG default, mainly Linux)
3. `$HOME/.tinyview/` (legacy default — used as the final fallback when none of the above exist, so existing setups keep working)

`config.toml` and the `templates/` dir both live under this root. Config is loaded **only when the
raw fast path is bypassed** (template, param, or file path is involved), so it cannot slow down
`echo … | tinyview`.

```toml
window_width = 1000
window_height = 760
default_template = "raw"

[extension]
md = "markdown"
markdown = "markdown"
mmd = "mermaid"
rs = "code"
ts = "code"

# default language for the `code` template (overridden by `--param lang=…`)
[templates.code.params]
lang = "rust"

# default theme for the `mermaid` template (default | dark | forest | neutral)
[templates.mermaid.params]
theme = "neutral"
```

Template resolution priority: **`--template` > extension mapping > `default_template` > `raw`**.

A user template (any name that is not a built-in) lives next to the config and overrides nothing
built-in:

```text
~/.tinyview/
├── config.toml
└── templates/
    └── my-layout.html   # used via `-t my-layout`
```

---

## Template system

TinyView has no template engine. A template is a single self-contained HTML file. The runtime
performs **one string replacement** that swaps the marker for a JSON literal:

```html
<!-- somewhere in <head> -->
<script>
  window.__TINYVIEW__ = /*__TINYVIEW__*/ null /*__TINYVIEW__*/;
</script>
```

After injection, the template sees:

```ts
window.__TINYVIEW__ = {
  input: string,                       // stdin / file contents / --html
  params: Record<string, string>,      // merged config + --param (CLI wins)
  title: string,
  path: string | null,                 // present for file input only
};
```

Template responsibilities:

- HTML-escape `input` on the template side (`element.textContent = …`).
- Inline all CSS and JS inside `<style>` / `<script>` — external `href` / `src` cannot resolve
  (no server).
- Do not declare a `<meta http-equiv="Content-Security-Policy">`; the runtime owns CSP.

A minimal template:

```html
<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <script>
      window.__TINYVIEW__ = /*__TINYVIEW__*/ null /*__TINYVIEW__*/;
    </script>
  </head>
  <body>
    <pre id="out"></pre>
    <script>
      document.getElementById("out").textContent = window.__TINYVIEW__.input;
    </script>
  </body>
</html>
```

Built-in templates: `raw` (no substitution, fastest path), `text` (escaped monospace),
`minimal` (centered HTML fragment shell), and the optional `markdown` / `mermaid` / `code`
(library bundled and inlined at render time).

---

## Permissions

TinyView denies by default: no fetch, no clipboard, no persistent storage, no DevTools (release),
no native bridge, no top-level external navigation. A strict CSP `<meta>` is injected at render
time except in `raw` mode (raw assumes trusted input — `--allow-*` flags re-enable CSP injection).

| Flag                | Effect                                                                                      |
| ------------------- | ------------------------------------------------------------------------------------------- |
| `--allow-fetch`     | Relax CSP `connect-src` from `'none'` to `https: http: ws: wss:`.                           |
| `--allow-clipboard` | Enable wry clipboard. **macOS caveat:** WKWebView exposes Cmd+C/V at the OS level and cannot be fully disabled. |
| `--allow-storage`   | Disable incognito; persist DataStore between runs.                                          |

> **Opaque-origin caveat (clipboard / storage).** TinyView injects HTML in-memory via
> `with_html` with no base URL, so the document runs in an **opaque origin**. The Clipboard
> API (`navigator.clipboard`) is secure-context-gated and `localStorage` throws
> `SecurityError` in an opaque origin — so on the in-memory path these are **not reachable
> from page JS even with the flags set**. `--allow-fetch` is unaffected because it is enforced
> purely through the CSP `<meta>`. This is verified by the E2E self-test (see *Contributing*).

---

## Architecture

`Rust + wry + tao`. The parent process reads input, resolves the template, performs the
`__TINYVIEW__` substitution, validates, then re-spawns itself (`Command::spawn` + `pre_exec`
`setsid` on Unix, `DETACHED_PROCESS` on Windows) and exits. The pre-composed HTML is piped to the
child's stdin so the no-temp-file invariant holds. The child opens the native WebView and runs the
event loop. `--watch` skips detach and re-renders on file change via `notify-debouncer-mini`.

### macOS: detached window behavior

The detached child is a bare Rust binary — there is no `.app` bundle and no `Info.plist`.
Despite that, the window behaves like a normal app window: `tao` defaults the activation policy
to `NSApplicationActivationPolicyRegular` and activates the app on launch, so the detached child

- **shows a Dock icon**,
- **takes keyboard focus** when the window opens, and
- **is included in the Cmd-Tab application switcher**.

TinyView additionally sets `ActivationPolicy::Regular` explicitly on the child's event loop so this
behavior is pinned to TinyView rather than left to a framework default.

Known gap: without a bundle, the app name shown in the menu bar / Cmd-Tab is the raw binary name
(`tinyview`) and there is no custom app icon. Packaging as a proper `.app` for full system
integration is tracked in [#11](https://github.com/TakakiAraki09/tiny-view/issues/11).

Full design and rationale: [`docs/PRD.md`](docs/PRD.md).

---

## Status

MVP is complete:

- stdin / file / `--html` input
- `raw` / `text` / `minimal` built-in templates
- user templates via `~/.tinyview/templates/`
- `--watch` with debounced reload
- detach-by-default on Unix (Command::spawn + setsid); Windows path scaffolded
- CSP injection, incognito-by-default, navigation handler, `--allow-*` flags
- `--frameless` and `--transparent` window flags
- `$XDG_CONFIG_HOME` / `~/.config/tinyview` config root with legacy `~/.tinyview` fallback

Not yet implemented / out of MVP scope: optional `markdown` / `mermaid` / `code` built-ins,
Windows runtime verification, published binaries,
`cargo install` / Homebrew distribution.

---

## Contributing

Read [`docs/PRD.md`](docs/PRD.md) first — it is the source of truth for what TinyView is and is
not. Proposals that add a server, a port, a temp preview file, or that hurt the <150 ms startup
target are out of scope by definition. Template / plugin / optional-feature paths are the right
home for anything beyond the core runtime.

### Tests

```bash
cargo test                  # unit tests (CSP construction, injection, CLI parsing)
```

The `--allow-*` flags are additionally covered by a **live-WebView E2E self-test** that drives a
real WebView through the production build path and reads page-side JS behavior back over a
feature-gated IPC channel (`src/e2e.rs`). The bridge it needs is compiled **only** under the `e2e`
feature, so the production binary never carries a JS→native bridge.

```bash
# macOS (authoritative):
TINYVIEW_E2E_SELFTEST=1 cargo run --features e2e
# Linux (needs a display — wrap in xvfb):
xvfb-run -a env TINYVIEW_E2E_SELFTEST=1 cargo run --features e2e
```

It exits non-zero on a hard failure. CI runs it best-effort on macOS + Linux (the `e2e` job);
headless GUI execution is environment-dependent, so local macOS is the source of truth.

---

## License

MIT. See [`LICENSE`](LICENSE).
