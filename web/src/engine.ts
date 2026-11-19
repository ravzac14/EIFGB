// The engine in the browser: a thin wrapper over the WebAssembly build of
// crates/eifgb-web. Every call takes and returns plain JS values.

export type Severity = "error" | "warning";
export interface Diagnostic {
  severity: Severity;
  /** Dotted source path, e.g. `objects.brass_key.in`. */
  path: string;
  message: string;
}
export type LineKind = "banner" | "title" | "text" | "notice" | "choice" | "ending" | "error";
export interface Line {
  kind: LineKind;
  text: string;
}
export interface Status {
  room: string;
  score: number;
  max_score: number | null;
  turns: number;
}
export interface Output {
  lines: Line[];
  status: Status;
  /** The open choice's options, numbered from 1. */
  options: string[];
  ended: "win" | "loss" | "neutral" | null;
  /** Which ending was reached (its key in `endings:`). */
  ending?: string | null;
  /** Set after SAVE: the whole game state. */
  save: string | null;
  restore_requested: boolean;
  quit_requested: boolean;
}
/** What the player has seen of the map (see `eifgb_map` in the bridge). */
export interface PlayerMap {
  here: string;
  rooms: { id: string; name: string }[];
  /** `to` is missing when the far side hasn't been visited yet. */
  exits: { from: string; dir: string; to?: string; door?: string; closed?: boolean }[];
}
export interface Meta {
  id: string;
  title: string;
  author: string | null;
  version: string;
  /** The game's own look (`presentation:` in game.yaml), if it chose one. */
  presentation?: Presentation | null;
  /** Every ending's key, from `endings:`. */
  endings?: string[];
  /** The standard library version the game was built with, the IR version and the engine's. */
  stdlib?: number;
  ir?: number;
  engine?: string;
}
export interface Presentation {
  theme?: string;
  accent?: string;
}
export type Files = Record<string, string>;
export interface Loaded {
  diagnostics: Diagnostic[];
  meta?: Meta;
  output?: Output;
}
export interface Engine {
  /** Compile `files` and start the game. */
  load(files: Files, entry?: string, seed?: number): Loaded;
  /** Compile only. */
  check(files: Files, entry?: string): { diagnostics: Diagnostic[] };
  /** One line of input. */
  input(line: string): Output;
  /** The whole game state, as a string. */
  save(): string;
  /** Put a saved state back. */
  restore(save: string): Output;
  /** The rooms visited so far and the ways out of them. */
  map(): PlayerMap;
}

interface Module {
  ccall(name: string, ret: "number", types: string[], args: unknown[]): number;
  UTF8ToString(ptr: number): string;
}

/**
 * Loads the engine from `base` (the folder holding eifgb.mjs and
 * eifgb_web.wasm, as a URL ending in `/`).
 */
export async function loadEngine(base: string | URL): Promise<Engine> {
  const url = new URL("eifgb.mjs", base).href;
  const create = (await import(/* @vite-ignore */ url)).default as (o?: object) => Promise<Module>;
  const m = await create();
  const call = (name: string, types: string[], args: unknown[]) =>
    JSON.parse(m.UTF8ToString(m.ccall(name, "number", types, args)));
  return {
    load: (files, entry = "game.yaml", seed = Date.now() % 2 ** 32) =>
      call("eifgb_load", ["string", "string", "number"], [JSON.stringify(files), entry, seed]),
    check: (files, entry = "game.yaml") => call("eifgb_check", ["string", "string"], [JSON.stringify(files), entry]),
    input: (line) => call("eifgb_input", ["string"], [line]),
    save: () => call("eifgb_save", [], []),
    restore: (save) => call("eifgb_restore", ["string"], [save]),
    map: () => call("eifgb_map", [], []),
  };
}
