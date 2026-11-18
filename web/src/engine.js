// The engine in the browser: a thin wrapper over the WebAssembly build
// (crates/eifgb-web). Every call takes and returns plain JS values.
import createEifgb from "./wasm/eifgb.mjs";

/**
 * Loads the engine. `options` are passed to Emscripten (e.g. `locateFile`).
 * @returns {Promise<Engine>}
 */
export async function loadEngine(options = {}) {
  const m = await createEifgb(options);
  const call = (name, types, args) => JSON.parse(m.UTF8ToString(m.ccall(name, "number", types, args)));
  return {
    /** Compile `files` ({path: text}) and start the game. → {diagnostics, output?} */
    load: (files, entry = "game.yaml", seed = Date.now() % 2 ** 32) =>
      call("eifgb_load", ["string", "string", "number"], [JSON.stringify(files), entry, seed]),
    /** Compile only. → {diagnostics} */
    check: (files, entry = "game.yaml") => call("eifgb_check", ["string", "string"], [JSON.stringify(files), entry]),
    /** One line of input. → output {lines, status, options, ended, save, restore_requested, quit_requested} */
    input: (line) => call("eifgb_input", ["string"], [line]),
    /** The current game state as a string. */
    save: () => call("eifgb_save", [], []),
    /** Restore a saved state. → output */
    restore: (save) => call("eifgb_restore", ["string"], [save]),
  };
}
