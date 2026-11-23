// The engine in the browser: a thin wrapper over the WebAssembly build of
// crates/eifgb-web. Every call takes and returns plain JS values.

export type Severity = "error" | "warning";
export interface Diagnostic {
  severity: Severity;
  /** Dotted source path, e.g. `objects.brass_key.in`. */
  path: string;
  message: string;
}
export type LineKind = "banner" | "title" | "text" | "notice" | "choice" | "ending" | "error" | "picture";
export interface Line {
  kind: LineKind;
  text: string;
  /** For a picture: its file in the project, or a web address. `text` says what it shows. */
  picture?: string;
}
export interface Status {
  room: string;
  score: number;
  max_score: number | null;
  turns: number;
  /** The time of day ("7:30 am"), when the game keeps time and shows it. */
  time?: string | null;
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
  /** Set when the command wasn't understood: why, for the editor to suggest a fix. */
  missed?: Missed;
}
export interface Missed {
  kind: "unknown_verb" | "unknown_word" | "scenery" | "not_here" | "partial" | "no_pronoun" | "other";
  word?: string;
  verb?: string;
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
  /** The game's language, like "en". */
  language?: string;
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
/** The puzzle chain, from the editor's check: see crates/eifgb-compiler/src/puzzles.rs. */
export interface Puzzles {
  /**
   * What the player does, each after what it `needs` (indexes into `steps`).
   * `or`: other ways to do it; `either`: for a choice, the others it rules
   * out; `script`: the script it leans on, whose inside the chain can't see.
   */
  steps: { kind: "start" | "find" | "unlock" | "reach" | "choose" | "ending"; id: string; text: string; needs?: number[]; or?: string[]; either?: string[]; script?: string }[];
  /** `needs`: every step the ending takes, in order; `only_if`: the either-or choices it depends on. */
  endings: { key: string; kind: "win" | "loss" | "neutral"; reachable: boolean; from: string[]; needs?: number[]; only_if?: string[] }[];
  problems: PuzzleNote[];
  /** Where a player could get stuck on the way to a win. */
  stuck?: PuzzleNote[];
  /** What the chain takes on trust: scripts it can't see inside, conditions it assumes come true. */
  assumes?: PuzzleNote[];
}
/** Something the puzzle chain says about a room, thing, character, conversation or ending. */
export interface PuzzleNote {
  id: string;
  what: "room" | "thing" | "character" | "choice" | "ending";
  message: string;
}
/**
 * How a walkthrough played, in the editor's check: see
 * crates/eifgb-engine/src/testing.rs. `goal`: the ending it's for (null:
 * any win); `never`: what it never reached.
 */
export interface Walkthrough {
  result: "win" | "loss" | "neutral" | "unfinished" | "error" | "reply";
  steps: number;
  line?: number;
  problem?: string | null;
  ending?: string | null;
  goal?: string | null;
  file?: string;
  never?: { rooms: string[]; things: string[]; options: { choice: string; node: string; option: number; text: string }[] };
}
/** What the wanderer found: see crates/eifgb-engine/src/testing.rs. */
export interface Wandered {
  error?: string;
  moves: number;
  games: number;
  errors: { at: number; command: string; text: string; commands: string[] }[];
  endings: Record<string, number>;
  rooms: [number, number];
  things: [number, number];
  quiet: { from: number; moves: number; room: string } | null;
  never: string[];
}
export interface Loaded {
  diagnostics: Diagnostic[];
  meta?: Meta;
  output?: Output;
}
export interface Engine {
  /** Compile `files` and start the game. */
  load(files: Files, entry?: string, seed?: number): Loaded;
  /** Compile only. */
  check(files: Files, entry?: string): { diagnostics: Diagnostic[]; walkthrough?: Walkthrough; walkthroughs?: Walkthrough[]; puzzles?: Puzzles };
  /** The wanderer: random moves on new games of `files`, apart from the game being played. */
  wander(files: Files, seed: number, moves: number, entry?: string): Wandered;
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
    wander: (files, seed, moves, entry = "game.yaml") => call("eifgb_wander", ["string", "string", "number", "number"], [JSON.stringify(files), entry, seed, moves]),
  };
}
