# Dot Matrix Cam

Turns your camera into a live dot-matrix / halftone image. Written in Rust, with one egui UI
that runs natively on macOS and, compiled to WebAssembly, in the browser.

```
crates/dotmatrix-core   the effect: pixels in → pixels out, no platform code
crates/dotmatrix-app    egui app: native macOS binary + WASM web app
web/                    index.html with a 3-line loader for the WASM app
```

Controls: dot colour, background colour, grid size (how many dots), dot size, contrast,
halftone vs LED mode, invert. On narrow screens (phones) the controls move into a
collapsible shelf below the video.

The round button in the top-right corner saves a PNG screenshot of the current frame
(Mac app: `~/Pictures/dot-matrix-<date>.png`; phones: the share sheet, e.g. Save Image;
other browsers: a file download).

## Prerequisites

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

## Native app (macOS)

```sh
cargo run --release -p dotmatrix-app
```

macOS asks for camera permission for the app you launch it from (Terminal, VS Code…).

## Web app

Live: https://mtryggvason.github.io/dot-matrix-cam/ (deployed from `main` by
`.github/workflows/pages.yml`).

Locally:

```sh
./scripts/build-web.sh
npx serve web        # then open the printed URL (http://localhost:3000)
```

## Planned: virtual camera for Meet, Zoom, FaceTime

To show up as a selectable camera in every app, macOS requires a CoreMediaIO Camera Extension:
a Swift system extension inside a signed `.app` in `/Applications`, which needs an Apple
Developer team ID. The Rust app will feed it processed frames.
