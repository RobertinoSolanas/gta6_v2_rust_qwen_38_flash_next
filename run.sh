#!/usr/bin/env bash
# Serve web/ on http://127.0.0.1:8090/ (build first with ./build.sh).
set -euo pipefail
cd "$(dirname "$0")"
PORT="${1:-8090}"
if [ ! -f web/pkg/city_app.js ]; then
  echo "web/pkg is missing — run ./build.sh first" >&2
  exit 1
fi
echo "Neon Bay  ->  http://127.0.0.1:${PORT}/"
exec python3 -m http.server "${PORT}" --directory web --bind 127.0.0.1
