#!/usr/bin/env bash
#
# bundle-macos.sh — package the `tinyview` release binary as a macOS `.app`.
#
# WHY THIS EXISTS (issue #11)
# ---------------------------
# `tinyview` ships as a bare Mach-O CLI binary. That is the *primary* artifact
# and stays unchanged. macOS, however, treats a bare binary as a CLI tool and
# limits some windowing/system-integration behavior (menu-bar app name is the
# raw binary name, no app icon, no Spotlight/bundle identity). For users who
# want a double-clickable launcher with proper bundle identity, this script
# wraps the *same* release binary in a `TinyView.app` bundle. The `.app` is a
# SECOND, parallel artifact — it is not a `--gui` wrapper and changes nothing
# about how `tinyview` (the CLI) is built or invoked.
#
# DESIGN NOTES (project absolute principles — see CLAUDE.md / docs/PRD.md)
# -----------------------------------------------------------------------
# * This is build-time packaging only. It adds zero runtime dependencies and
#   does not touch the raw-path startup budget (<150ms) or the binary size
#   budget (<10MB): the binary inside the bundle is byte-identical to
#   target/release/tinyview.
# * No server / no port / no generated preview file: a `.app` bundle is just a
#   directory layout + Info.plist + a copy of the binary. None of that violates
#   the ephemeral-runtime / no-server contract.
# * LSUIElement = true: the bundle declares itself an "accessory" app (decided
#   in issue #11). At runtime the WebView event loop calls
#   `set_activation_policy(ActivationPolicy::Regular)` (src/main.rs), which
#   promotes the process so the window still shows a Dock icon, takes focus and
#   joins Cmd-Tab. So double-clicking TinyView.app opens a normal, focusable
#   WebView window while the bundle's resting identity remains accessory.
#
# USAGE
# -----
#   scripts/bundle-macos.sh [--binary <path>] [--out-dir <dir>] [--no-build]
#
#   --binary <path>   Use an existing binary instead of building one.
#                     Default: target/release/tinyview (built if missing).
#   --out-dir <dir>   Where to place TinyView.app. Default: target/release.
#   --no-build        Do not run `cargo build --release`; require the binary to
#                     already exist (used by CI where the release build is a
#                     separate step).
#
# Output: <out-dir>/TinyView.app
#
# BUNDLE EXECUTABLE vs EMBEDDED CLI
# ---------------------------------
# A `.app` launched from Finder gets no stdin, no file arg and no --html, so the
# raw `tinyview` binary would exit with "no input" and never open a window —
# failing the issue #11 acceptance test ("double-click opens a WebView"). To fix
# that *without* changing the CLI (issue #11: "tinyview (CLI) は据え置き"), the
# bundle's CFBundleExecutable is a tiny launcher (Contents/MacOS/tinyview-app) that
# pipes a bundled welcome HTML (Contents/Resources/welcome.html) into the real
# binary (Contents/MacOS/tinyview) in --foreground mode. Welcome content is
# in-memory via stdin (no temp preview file, no server). When the CLI binary is
# invoked directly from a shell it behaves exactly as before — the launcher only
# governs the double-click entry point.
#
set -euo pipefail

# --- This script only makes sense on / for macOS bundles. -------------------
if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "warning: bundle-macos.sh produces a macOS .app; running on $(uname -s)." >&2
  echo "         The layout is still generated, but it is only meaningful on macOS." >&2
fi

# --- Locate the repo root so the script works from any CWD. -----------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# The embedded CLI binary keeps its familiar name. The bundle's launcher (the
# CFBundleExecutable that Finder runs on double-click) is a separate file so it
# can supply default input; see "BUNDLE EXECUTABLE vs EMBEDDED CLI" above.
# NOTE: the launcher name must NOT collide with BIN_NAME on a case-insensitive
# filesystem (macOS default). "TinyView" vs "tinyview" would be the *same* path
# and silently clobber the embedded binary, so the launcher uses a distinct
# suffix.
BIN_NAME="tinyview"
LAUNCHER_NAME="tinyview-app"
APP_NAME="TinyView"
BUNDLE_ID="com.tinyview.app"

BINARY=""
OUT_DIR="${REPO_ROOT}/target/release"
DO_BUILD=1

while [[ $# -gt 0 ]]; do
  case "$1" in
    --binary)
      BINARY="$2"; shift 2 ;;
    --out-dir)
      OUT_DIR="$2"; shift 2 ;;
    --no-build)
      DO_BUILD=0; shift ;;
    -h|--help)
      sed -n '2,40p' "${BASH_SOURCE[0]}"; exit 0 ;;
    *)
      echo "error: unknown argument: $1" >&2; exit 2 ;;
  esac
done

# --- Resolve the binary to embed. -------------------------------------------
if [[ -z "${BINARY}" ]]; then
  BINARY="${REPO_ROOT}/target/release/${BIN_NAME}"
  if [[ ! -f "${BINARY}" ]]; then
    if [[ "${DO_BUILD}" -eq 1 ]]; then
      echo "==> release binary not found; building (cargo build --release)"
      ( cd "${REPO_ROOT}" && cargo build --release )
    else
      echo "error: ${BINARY} not found and --no-build was given." >&2
      exit 1
    fi
  fi
fi

if [[ ! -f "${BINARY}" ]]; then
  echo "error: binary to bundle does not exist: ${BINARY}" >&2
  exit 1
fi

# --- Read the version straight from Cargo.toml so it never drifts. ----------
VERSION="$(
  awk -F' *= *' '/^\[package\]/{p=1;next} /^\[/{p=0} p&&$1=="version"{gsub(/"/,"",$2);print $2;exit}' \
    "${REPO_ROOT}/Cargo.toml"
)"
VERSION="${VERSION:-0.0.0}"

# --- Build the bundle layout. -----------------------------------------------
APP_DIR="${OUT_DIR}/${APP_NAME}.app"
CONTENTS_DIR="${APP_DIR}/Contents"
MACOS_DIR="${CONTENTS_DIR}/MacOS"
RESOURCES_DIR="${CONTENTS_DIR}/Resources"

echo "==> creating ${APP_DIR}"
rm -rf "${APP_DIR}"
mkdir -p "${MACOS_DIR}" "${RESOURCES_DIR}"

# Embed the real CLI binary (unchanged) under its familiar name.
cp "${BINARY}" "${MACOS_DIR}/${BIN_NAME}"
chmod +x "${MACOS_DIR}/${BIN_NAME}"

# Bundled welcome page shown on double-click. Self-contained HTML (inline CSS,
# no external href/src) per the No-Server contract. Follows OS dark mode.
cat > "${RESOURCES_DIR}/welcome.html" <<'WELCOME'
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>TinyView</title>
    <style>
      :root { color-scheme: light dark; }
      html, body { height: 100%; margin: 0; }
      body {
        display: flex; align-items: center; justify-content: center;
        font: 15px/1.6 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
        background: #fff; color: #1d1d1f;
      }
      @media (prefers-color-scheme: dark) {
        body { background: #1d1d1f; color: #f5f5f7; }
        code, pre { background: #2c2c2e; }
      }
      main { max-width: 640px; padding: 2.5rem; text-align: center; }
      h1 { font-size: 2rem; margin: 0 0 .25rem; }
      p.tagline { opacity: .7; margin: 0 0 1.75rem; }
      pre {
        text-align: left; background: #f2f2f7; border-radius: 8px;
        padding: 1rem 1.25rem; overflow-x: auto; font-size: 13px;
      }
      code { font-family: "SF Mono", Menlo, Consolas, monospace; }
      a { color: #0a84ff; }
    </style>
  </head>
  <body>
    <main>
      <h1>TinyView</h1>
      <p class="tagline">Ephemeral CLI WebView runtime — render Web UI from a pipe.</p>
      <p>This window confirms the macOS app bundle works. TinyView is driven from the command line:</p>
      <pre><code>echo '&lt;h1&gt;Hello&lt;/h1&gt;' | tinyview
tinyview app.html
tinyview README.md -t markdown</code></pre>
      <p>See the <a href="https://github.com/TakakiAraki09/tiny-view">project README</a> for the full CLI.</p>
    </main>
  </body>
</html>
WELCOME

# Launcher = CFBundleExecutable. On double-click Finder runs this; it pipes the
# welcome page into the real binary in --foreground so the bundle process owns
# the WebView window (Finder/launchd expect the app process to persist while a
# window is open; the CLI's detach-and-exit default would look like an instant
# quit). $0 resolves to .../Contents/MacOS/tinyview-app, so siblings are addressed
# relative to it — the bundle is location-independent.
cat > "${MACOS_DIR}/${LAUNCHER_NAME}" <<LAUNCHER
#!/bin/sh
# TinyView.app launcher (generated by scripts/bundle-macos.sh). Opens the
# bundled welcome page; the CLI binary itself is untouched.
HERE="\$(cd "\$(dirname "\$0")" && pwd)"
RESOURCES="\$(cd "\${HERE}/../Resources" && pwd)"
exec "\${HERE}/${BIN_NAME}" --foreground < "\${RESOURCES}/welcome.html"
LAUNCHER
chmod +x "${MACOS_DIR}/${LAUNCHER_NAME}"

# Optional app icon. The repo ships no .icns today (issue #11 marks it
# optional); if one is added at assets/AppIcon.icns it is picked up here.
ICON_KEY=""
ICON_SRC="${REPO_ROOT}/assets/AppIcon.icns"
if [[ -f "${ICON_SRC}" ]]; then
  cp "${ICON_SRC}" "${RESOURCES_DIR}/AppIcon.icns"
  ICON_KEY=$'\t<key>CFBundleIconFile</key>\n\t<string>AppIcon</string>\n'
  echo "==> embedded icon: ${ICON_SRC}"
else
  echo "==> no icon at ${ICON_SRC} (optional, skipping)"
fi

# --- Info.plist -------------------------------------------------------------
# LSUIElement=true  -> accessory app (issue #11 decision).
# LSMinimumSystemVersion=10.13 mirrors the WKWebView baseline wry targets.
cat > "${CONTENTS_DIR}/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleName</key>
	<string>${APP_NAME}</string>
	<key>CFBundleDisplayName</key>
	<string>${APP_NAME}</string>
	<key>CFBundleIdentifier</key>
	<string>${BUNDLE_ID}</string>
	<key>CFBundleExecutable</key>
	<string>${LAUNCHER_NAME}</string>
	<key>CFBundleVersion</key>
	<string>${VERSION}</string>
	<key>CFBundleShortVersionString</key>
	<string>${VERSION}</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
${ICON_KEY}	<key>LSUIElement</key>
	<true/>
	<key>LSMinimumSystemVersion</key>
	<string>10.13</string>
	<key>NSHighResolutionCapable</key>
	<true/>
</dict>
</plist>
PLIST

echo "==> done: ${APP_DIR}"
echo "    open it with:  open \"${APP_DIR}\""
