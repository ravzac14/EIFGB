# EIFGB on the web

The engine, compiler and Lua, compiled to WebAssembly, and the pages that
use them. The bridge is the `eifgb-web` crate (`crates/eifgb-web`);
`src/engine.js` wraps it for JavaScript.

## Building

One-time setup:

```sh
rustup target add wasm32-unknown-emscripten
git clone https://github.com/emscripten-core/emsdk.git ~/emsdk
~/emsdk/emsdk install latest && ~/emsdk/emsdk activate latest
```

Then build the engine into `web/src/wasm/` (not committed):

```sh
web/scripts/build-wasm.sh
```

## Trying it

```sh
node web/test/walkthroughs.mjs      # plays every example to a win
python3 -m http.server 8765         # from the repository root
```

and open <http://localhost:8765/web/> (or `?example=blackfang`).

## The engine API (`src/engine.js`)

- `loadEngine()`: loads the WebAssembly; resolves to an engine.
- `engine.load(files, entry, seed)`: compiles `files` (`{path: text}`) and
  starts the game: `{ diagnostics, output? }`.
- `engine.check(files, entry)`: compiles only: `{ diagnostics }`.
- `engine.input(line)`: one command: the output (`lines`, `status`,
  `options`, `ended`, `save`, `restore_requested`, `quit_requested`).
- `engine.save()` / `engine.restore(save)`: the whole game state as a
  string, and putting it back.
