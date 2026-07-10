# Coleoptera (Rust single-binary rewrite)

A standalone desktop app that crawls a URL and converts the page to clean
Markdown. This is a **complete rewrite** of the original Python (Flask +
crawl4ai) backend + Electron/React frontend into a **single native executable**
built with Rust + Tauri v2.

One binary. One window. No Python, no Node, no Electron, no external browser.

## What changed vs. the original

| Original (Python/Electron)        | This rewrite (Rust/Tauri)                       |
|-----------------------------------|-------------------------------------------------|
| `backend.py` Flask SSE on :5001   | Embedded Axum HTTP server (same binary)         |
| Electron + React + Vite           | Native webview window, single `index.html` UI   |
| crawl4ai = Playwright Chromium    | `reqwest` HTTP fetch + `html2md` (HTML→Markdown)|
| Two runtimes, a venv, npm         | One `cargo build`, one executable               |

## Why no browser engine?

The option chosen was a **native desktop window with a real single binary, no
Electron**. To keep it a *true* single binary that runs anywhere with no
Chromium download, the crawl engine uses plain HTTP fetch + HTML→Markdown
conversion instead of a headless browser.

Consequence: **JS-rendered / SPA pages won't have their client-rendered DOM
captured** (the source HTML is what gets converted). Static sites, articles,
docs, and server-rendered pages work great. This is the same trade-off as the
"fully self-contained" option — if you need full JS rendering, that requires
bundling a Chromium binary (the `chromiumoxide` crate), which breaks the
single-binary property.

The crawler is isolated in `src-tauri/src/crawler.rs` behind a clean function
signature, so swapping in a Chromium-backed engine later is a localized change.

## Features

- URL input → clean Markdown output (GFM-style: headings, lists, code, tables,
  blockquotes, links).
- Real-time **SSE** progress events: loading → loaded → extracting → done.
- Live log stream with info/warning/error levels.
- **Stop** button cancels a crawl mid-flight (server-side abort).
- Dark UI, **embedded directly into the binary** (`include_str!`) — no external
  asset files, no CDN, no network at runtime.
- `/health` and `/crawl/cancel` endpoints mirror the original API surface.

## How the single binary works

There are exactly two processes and they share one origin:

1. On launch, the Tauri app spawns an **embedded Axum HTTP server** on
   `127.0.0.1:1420` (the backend — crawl engine + SSE API).
2. A native **webview window** then loads `http://127.0.0.1:1420/` (the
   frontend — `dist/index.html`, served from the same binary).

Because frontend and backend are same-origin, the UI calls the API with plain
`fetch` — no CORS, no separate server process, no config. `cargo tauri build`
packages the binary plus the native webview bootstrap into a real
`.app` / `.dmg` / `.exe` / `.AppImage`.

## Build

Requires: Rust stable + the platform's native webview deps. On macOS you need
Xcode Command Line Tools (`xcode-select --install`). The Tauri CLI is optional
— plain `cargo` works.

```bash
# Debug build / run
cd src-tauri
cargo run            # launches the native window

# Release (single optimized binary)
cargo build --release
# Output: src-tauri/target/release/coleoptera   (on macOS rename to .app-less
# executable, or use tauri build for a proper .app/.dmg bundle)
```

### Proper app bundle (.app / .dmg / .exe / .AppImage)

```bash
cargo install tauri-cli --version "^2"
cd src-tauri
cargo tauri build   # produces platform installers in src-tauri/target/release/bundle/
```

## Project structure

```
Coleoptera/
├── dist/index.html          # Single-file frontend UI (dark, SSE + md render)
├── src-tauri/
│   ├── Cargo.toml
│   ├── build.rs
│   ├── tauri.conf.json
│   ├── icons/               # App icons (png/icns/ico)
│   ├── examples/e2e.rs      # Standalone end-to-end test (cargo run --example e2e)
│   └── src/
│       ├── main.rs          # Binary entry
│       ├── lib.rs           # Tauri builder: spawns Axum, opens webview window
│       ├── crawler.rs       # Crawl engine + SSE events (HTTP fetch + HTML→Markdown)
│       ├── server.rs        # Axum routes: /, /health, /crawl/stream, /crawl/cancel
│       └── state.rs         # In-flight crawl sessions + cancellation
└── README.md
```

## API (same-origin, embedded)

- `POST /crawl/stream` — body `{"url":"...","options":{}}`. Returns an SSE
  stream of events: `start`, `progress`, `log`, `done`.
- `POST /crawl/cancel` — body `{"session_id":"..."}` to abort a running crawl.
- `GET /health` — `{"status":"ok"}`.
- `GET /ws` — optional WebSocket bridge (same event protocol as SSE).

## License

MIT.
