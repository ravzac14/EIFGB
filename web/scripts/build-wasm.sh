#!/usr/bin/env bash
# Builds the engine, compiler and Lua for the browser, into web/src/wasm/.
# Needs the Rust target (rustup target add wasm32-unknown-emscripten) and the
# Emscripten SDK (source ~/emsdk/emsdk_env.sh, or set EMSDK).
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
if ! command -v emcc >/dev/null; then
  # shellcheck disable=SC1091
  source "${EMSDK:-$HOME/emsdk}/emsdk_env.sh" >/dev/null 2>&1
fi
cd "$root"
cargo build -p eifgb-web --release --target wasm32-unknown-emscripten
out="$root/web/src/wasm"
mkdir -p "$out"
cp target/wasm32-unknown-emscripten/release/eifgb-web.js "$out/eifgb.mjs"
cp target/wasm32-unknown-emscripten/release/eifgb_web.wasm "$out/eifgb_web.wasm"
ls -la "$out"
