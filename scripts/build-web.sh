#!/usr/bin/env bash
# Build the egui app to WebAssembly for the web page.
set -euo pipefail
cd "$(dirname "$0")/.."
wasm-pack build crates/dotmatrix-app --release --target web --out-dir ../../web/pkg --no-typescript
echo "Built: web/ (npx serve web)"
