# Coleoptera

> Turn any website into clean Markdown — one binary, no browser, no runtime dependencies.

Coleoptera is a desktop application that crawls a URL and converts the rendered
page into clean, readable Markdown. It is built as a **single native
executable**: a Rust [Tauri] backend that embeds an [Axum] HTTP server (the
crawl engine + a Server-Sent-Events API) and serves a bundled web UI in a
native webview window.

There is no Python, no Node, no Electron, and no external browser required at
runtime. One binary does it all.

---

## Features

- **URL → Markdown** — fetch a page and convert it to GitHub-flavored Markdown
  (headings, lists, code blocks, tables, blockquotes, links).
- **Live progress** — real-time SSE events (`loading → loaded → extracting →
  done`) streamed to the UI as the crawl runs.
- **Live log** — an info / warning / error log pane mirrors every step.
- **Stop anytime** — cancel an in-flight crawl with a single click; the server
  aborts the request on its side.
- **Self-contained UI** — the entire frontend is embedded into the binary
  (`include_str!`), so there are no asset files, no CDN, and no network needed
  to render it.
- **Simple API** — `/health`, `/crawl/stream`, and `/crawl/cancel` mirror the
  original service surface.

---

## How it works

A Coleoptera process is two cooperating pieces that share a single origin:

1. On launch, the app starts an **embedded Axum server** on
   `http://127.0.0.1:1420` — this is the backend (crawl engine + SSE API).
2. A native **webview window** then loads `http://127.0.0.1:1420/`, which is
   the frontend (`dist/index.html`, served straight from the binary).

Because the frontend and backend are same-origin, the UI talks to the API with
plain `fetch` — no CORS, no separate server process, no configuration.

```text
┌─────────────────────────────────────────────┐
│  Coleoptera (single native executable)       │
│                                               │
│  ┌──────────────┐        ┌────────────────┐  │
│  │  Axum server │◀─SSE───│  webview window │  │
│  │  :1420       │        │  (bundled UI)   │  │
│  │  crawl + MD  │        └────────────────┘  │
│  └──────────────┘                            │
└─────────────────────────────────────────────┘
```

---

## Download & run

Grab a prebuilt bundle from the latest GitHub Release:

- **macOS** — `Coleoptera_1.0.0_aarch64.dmg` (or the `.app` inside)
- **Linux** — `.AppImage` / `.deb`
- **Windows** — `.exe` installer

On macOS, if you see an "unidentified developer" warning, right-click →
**Open** the first time (no signing certificate is bundled in the open-source
build).

To run a locally built copy:

```bash
open src-tauri/target/release/bundle/macos/Coleoptera.app
```

---

## Build from source

### Prerequisites

- [Rust](https://www.rust-lang.org/) (stable)
- Platform webview tooling:
  - **macOS** — Xcode Command Line Tools (`xcode-select --install`)
  - **Linux** — `webkit2gtk-4.1` dev packages
  - **Windows** — the WebView2 runtime (preinstalled on Win 11)
- *(optional)* the [Tauri CLI] for producing installers —
  `cargo install tauri-cli --version "^2"`

### Run in debug mode

```bash
cd src-tauri
cargo run
```

This compiles and launches the native window directly.

### Build a distributable

```bash
cd src-tauri
cargo tauri build
```

Artifacts land in `src-tauri/target/release/bundle/`:

```text
bundle/
├── macos/Coleoptera.app
├── dmg/Coleoptera_1.0.0_aarch64.dmg
├── deb/…               (Linux)
└── msi/…               (Windows)
```

---

## Usage

1. Launch Coleoptera.
2. Type or paste a URL into the address bar (e.g. `https://example.com`).
   `http://` / `https://` are added automatically if omitted.
3. Click **Crawl**. The Markdown output appears on the left; a live log
   streams on the right.
4. Click **Stop** to abort a crawl in progress.

### API (same-origin, embedded)

| Method | Path              | Description                                                        |
|--------|-------------------|--------------------------------------------------------------------|
| `POST` | `/crawl/stream`   | Body `{"url":"…","options":{}}`. Returns an SSE stream of events: `start`, `progress`, `log`, `done`. |
| `POST` | `/crawl/cancel`   | Body `{"session_id":"…"}` to abort a running crawl.               |
| `GET`  | `/health`         | Returns `{"status":"ok"}`.                                        |

A minimal SSE `done` event:

```json
{
  "event": "done",
  "success": true,
  "markdown": "# Example Domain\n\nThis domain is for use in …"
}
```

---

## Project structure

```text
Coleoptera/
├── dist/index.html            # Single-file frontend UI (dark, SSE + Markdown render)
├── src-tauri/
│   ├── Cargo.toml             # Rust workspace manifest
│   ├── build.rs
│   ├── tauri.conf.json        # Tauri v2 app + bundle config
│   ├── icons/                 # App icons (png / icns / ico)
│   ├── examples/
│   │   └── e2e.rs             # Standalone end-to-end test (cargo run --example e2e)
│   └── src/
│       ├── main.rs            # Binary entry point
│       ├── lib.rs             # Tauri builder: spawns Axum, opens the webview window
│       ├── crawler.rs         # Crawl engine + SSE events (HTTP fetch + HTML→Markdown)
│       ├── server.rs          # Axum routes: /, /health, /crawl/stream, /crawl/cancel
│       └── state.rs           # In-flight crawl sessions + cancellation
├── README.md
└── LICENSE                    # MIT
```

---

## Notes & limitations

The crawl engine uses an HTTP fetch plus an HTML→Markdown conversion rather
than a full headless browser. This keeps the app a **true single binary** that
runs anywhere with no Chromium download. The trade-off:

- **Works great** for static sites, articles, documentation, and
  server-rendered pages.
- **JS-rendered / SPA pages** may not capture client-rendered DOM, since the
  source HTML (not the post-hydration DOM) is what gets converted.

> The crawler lives behind a small, well-isolated function in
> `src-tauri/src/crawler.rs`. Swapping in a headless-Chromium backend (e.g. the
> `chromiumoxide` crate) later is a localized change, at the cost of shipping a
> browser binary.

---

## License

MIT — see [LICENSE](LICENSE).

[Tauri]: https://tauri.app/
[Axum]: https://github.com/tokio-rs/axum
[Tauri CLI]: https://v2.tauri.app/start/cli/
