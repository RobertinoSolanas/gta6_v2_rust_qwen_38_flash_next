#!/usr/bin/env bash
# Full gate: native build + all native tests + wasm build.
set -euo pipefail
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"

echo "== cargo build --workspace =="
cargo build --workspace

echo "== cargo test --workspace =="
cargo test --workspace

echo "== wasm-pack build =="
./build.sh --release

echo "check.sh: OK"
