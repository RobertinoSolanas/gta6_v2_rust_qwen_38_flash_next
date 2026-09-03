#!/usr/bin/env bash
# Build the wasm package (writes web/pkg/) and copy the shell page.
set -euo pipefail
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"
wasm-pack build crates/city-app --target web --out-dir ../../web/pkg --out-name city_app "$@"
