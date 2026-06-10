# TinyView

> Ephemeral CLI WebView runtime — render Web UI from a pipe, no server, no port, no temp file.

```bash
echo '<h1>Hello</h1>' | tinyview
```

A native WebView window opens, the HTML paints, you close it, nothing persists. TinyView is the
Web UI counterpart of `open file.png` — a one-shot rendering primitive, not a browser.

---

## Download

```bash
cargo install tinyview
```

This builds and installs the `tinyview` binary into `~/.cargo/bin` (make sure it is on your
`PATH`). Requires Rust 1.75+. On Linux you also need WebKitGTK development headers at build time
(`libwebkit2gtk-4.1-dev` on Debian/Ubuntu).

Prefer not to use `cargo`? Build [from source](#from-source), or on macOS package a
double-clickable [`TinyView.app`](#macos-tinyviewapp-bundle-optional). See [**Install**](#install)
for all options.

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

### From crates.io (recommended)

```bash
cargo install tinyview
```

This builds and installs the `tinyview` binary into `~/.cargo/bin` (make sure it is on your `PATH`).
On Linux you also need WebKitGTK development headers at build time
(`libwebkit2gtk-4.1-dev` on Debian/Ubuntu).

### From source

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

### macOS: TinyView.app bundle (optional)

The CLI above is the primary artifact. On macOS you can additionally package the
**same** release binary as a double-clickable `TinyView.app`:

```bash
cargo build --release
scripts/bundle-macos.sh          # → target/release/TinyView.app
open target/release/TinyView.app # double-click equivalent: opens a welcome WebView
```

The bundle is a *second, parallel* artifact — it does not replace or wrap the
`tinyview` CLI, which is still invoked exactly as documented below. The binary
embedded in `Contents/MacOS/tinyview` is byte-identical to the CLI build, so the
bundle adds nothing to startup time or binary size.

What the bundle adds is macOS bundle identity: a `CFBundleIdentifier`, an app
name in the menu bar / Cmd-Tab, `Info.plist` metadata, and (if you drop one at
`assets/AppIcon.icns`) an app icon. It declares `LSUIElement = true`, so the
bundle's resting identity is an *accessory* app; at runtime TinyView promotes
itself to a regular activation policy (`src/main.rs`) so the WebView window
still shows a Dock icon, takes focus, and joins Cmd-Tab.

Because a `.app` launched from Finder gets no stdin/file/`--html`, its
`CFBundleExecutable` is a tiny launcher (`Contents/MacOS/tinyview-app`) that
pipes a bundled welcome page (`Contents/Resources/welcome.html`) into the binary
in `--foreground` mode. No temp file and no server are involved — the welcome
HTML is fed in-memory over stdin, same as `echo … | tinyview`.

CI builds and uploads `TinyView.app` as the `TinyView-app-macos` artifact on the
`macos-latest` runner (see `.github/workflows/ci.yml`).

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
| `--allow-clipboard`   | Enable clipboard API in the WebView. See *Permissions* below.        |
| `--allow-storage`     | Persist WebView storage (disables incognito mode).                   |
| `-h, --help`          | Show usage.                                                          |
| `-V, --version`       | Show version.                                                        |

There is no CLI flag for outbound `fetch`: a document grants itself network access with
`<meta name="tinyview-allow" content="fetch">` in its `<head>`. See *Permissions* below.

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
time except in `raw` mode (raw assumes trusted input — `--allow-*` flags or the
`tinyview-allow` meta re-enable CSP injection).

| Flag                | Effect                                                                                      |
| ------------------- | ------------------------------------------------------------------------------------------- |
| `--allow-clipboard` | Enable wry clipboard. **macOS caveat:** WKWebView exposes Cmd+C/V at the OS level and cannot be fully disabled. |
| `--allow-storage`   | Disable incognito; persist DataStore between runs.                                          |

> **Opaque-origin caveat (clipboard / storage).** TinyView injects HTML in-memory via
> `with_html` with no base URL, so the document runs in an **opaque origin**. The Clipboard
> API (`navigator.clipboard`) is secure-context-gated and `localStorage` throws
> `SecurityError` in an opaque origin — so on the in-memory path these are **not reachable
> from page JS even with the flags set**. The fetch grant is unaffected because it is enforced
> purely through the CSP `<meta>`. This is verified by the E2E self-test (see *Contributing*).

> **Granting fetch.** Outbound fetch has no CLI flag. A template or input document opts into it by
> putting `<meta name="tinyview-allow" content="fetch">` in its `<head>` — the runtime then relaxes
> CSP `connect-src` from `'none'` to `https: http: ws: wss:`. Content declares its own needs:
> since TinyView only renders HTML you piped or pointed it at, self-declaration cannot cross the
> trust boundary. `content` is a space-separated token list; only `fetch` is recognized today.
> Clipboard/storage stay CLI-only. See PRD §19.2.1.

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

When run as the bare CLI binary, the app name shown in the menu bar / Cmd-Tab is the raw binary
name (`tinyview`) and there is no custom app icon. For full system integration, package the binary
as a `TinyView.app` bundle (`LSUIElement = true` accessory app) via `scripts/bundle-macos.sh` — see
[*Install → macOS: TinyView.app bundle*](#macos-tinyviewapp-bundle-optional). The bundle is a
separate artifact; the CLI is unchanged.

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
- macOS `TinyView.app` bundle packaging (`scripts/bundle-macos.sh`, accessory app)

Distributed on crates.io (`cargo install tinyview`). Not yet implemented / out of MVP scope:
Windows runtime verification, pre-built signed release binaries, Homebrew distribution.

---

## Contributing

Read [`docs/PRD.md`](docs/PRD.md) first — it is the source of truth for what TinyView is and is
not. Proposals that add a server, a port, a temp preview file, or that hurt the <150 ms startup
target are out of scope by definition. Template / plugin / optional-feature paths are the right
home for anything beyond the core runtime.

### Releasing

Releases are automated with [release-plz](https://release-plz.dev) — see
[`.github/workflows/release.yml`](.github/workflows/release.yml). You never bump the version, edit
`Cargo.lock`, or create tags by hand; release-plz owns all three.

**Commit messages drive the version.** Commits merged to `main` must follow
[Conventional Commits](https://www.conventionalcommits.org), because release-plz derives the next
semver from them:

| Commit                                       | Bump                                                  |
| -------------------------------------------- | ----------------------------------------------------- |
| `fix: …`                                     | patch                                                 |
| `feat: …`                                    | minor                                                 |
| `feat!: …` or a `BREAKING CHANGE:` footer    | major                                                 |
| anything else (`chore:` / `docs:` / `ci:` / `refactor:` / `test:`, or non-conventional) | patch (default) |

By default release-plz treats everything that isn't `feat` or breaking as a patch, so even a
`chore:`-only batch yields a patch Release PR (all commits still appear in the changelog). The
strongest bump among the unreleased commits wins, and `cargo-semver-checks` forces a major bump if
the public API actually breaks regardless of the commit type.

**Cutting a release:**

1. Push to `main`. release-plz keeps a single open **Release PR** that bumps the version, updates
   `CHANGELOG.md`, and syncs `Cargo.lock`.
2. Merge that Release PR when you want to release. The merge publishes: release-plz `cargo publish`es
   the new version to crates.io and creates the git tag + GitHub Release automatically.

Merging the Release PR **is** the release gate — it's a deliberate human action, so it doubles as
the explicit step for the irreversible crates.io upload (a crates.io version can never be
re-published). The bump and `Cargo.lock` sync are still reviewed in the PR before the merge triggers
publishing.

> Repo setup (one-time): release-plz needs **Settings → Actions → General → "Allow GitHub
> Actions to create and approve pull requests"** enabled (so it can open the Release PR), plus a
> `CARGO_REGISTRY_TOKEN` secret used for publishing.

### MSRV policy

TinyView declares a **Minimum Supported Rust Version (MSRV)** via `rust-version` in
[`Cargo.toml`](Cargo.toml). It is the oldest Rust toolchain guaranteed to compile the crate, and
CI enforces it: the `msrv` job (in [`.github/workflows/ci.yml`](.github/workflows/ci.yml)) runs
`cargo build` with exactly that toolchain on every push and PR. If a dependency change raises the
required Rust, this job fails instead of letting the drift reach downstream consumers.

- **What sets the MSRV.** It is the *highest* `rust-version` across the resolved dependency graph,
  not a free choice. Today the floor comes from the native-WebView core (`wry`) and its transitive
  deps — not from TinyView's own source. You can find the current floor with `cargo metadata`.
- **When you may bump it.** Only when adding or updating a dependency genuinely requires a newer
  Rust, or when a language/`std` feature TinyView needs lands in a newer release. Do **not** bump
  the MSRV casually to use a nicety that has an MSRV-compatible alternative.
- **How to bump it.** Change `rust-version` in `Cargo.toml` **and** the matching `toolchain:`
  value (and the `name:`) in the `msrv` CI job in the same commit, so the two never drift apart.
  Note the reason in the PR description (which dependency/feature forced it).
- **Prefer not bumping.** If a dependency upgrade is what raises the MSRV and the upgrade is not
  required, pin the dependency to the last MSRV-compatible version instead of raising the floor.

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
