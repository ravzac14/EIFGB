// Everything the site keeps in the browser: settings, games you've loaded,
// autosaves and save slots. Storage can be missing or full (private
// windows, cleared data), so every call copes with that.

import type { Files } from "./engine.ts";

const P = "eifgb:";

function read<T>(key: string, fallback: T): T {
  try {
    const v = localStorage.getItem(P + key);
    return v === null ? fallback : (JSON.parse(v) as T);
  } catch {
    return fallback;
  }
}

/** False if it couldn't be stored (full, or storage is off). */
function write(key: string, value: unknown): boolean {
  try {
    localStorage.setItem(P + key, JSON.stringify(value));
    return true;
  } catch {
    return false;
  }
}

function remove(key: string) {
  try {
    localStorage.removeItem(P + key);
  } catch {
    /* nothing to do */
  }
}

// ------------------------------------------------------------- settings

export type Theme = "auto" | "light" | "dark" | "sepia" | "parchment" | "terminal" | "amber" | "contrast";
export type Size = "small" | "medium" | "large" | "xlarge";
export type Face = "serif" | "sans" | "legible";
export type Spacing = "normal" | "relaxed";
export type Width = "narrow" | "medium" | "wide";
/** How new text appears in the player: all at once, or typed out letter by letter. */
export type Reveal = "instant" | "typewriter";
/** Whether the player reads new text aloud, and how fast. */
export type Speech = "off" | "on" | "slow" | "fast";
export interface Settings {
  theme: Theme;
  size: Size;
  face: Face;
  spacing: Spacing;
  width: Width;
  reveal: Reveal;
  speech: Speech;
}
const DEFAULTS: Settings = { theme: "auto", size: "medium", face: "serif", spacing: "normal", width: "medium", reveal: "instant", speech: "off" };

export const settings = {
  get: (): Settings => ({ ...DEFAULTS, ...read<Partial<Settings>>("settings", {}) }),
  set: (s: Settings) => write("settings", s),
};

/**
 * The look the game being played asked for (`presentation:`), if any. It
 * applies while the player's own colours are left on "auto".
 */
let gameLook: { theme?: string; accent?: string } | null = null;
export function setGameLook(look: { theme?: string; accent?: string } | null | undefined) {
  gameLook = look?.theme || look?.accent ? look : null;
  applySettings();
}
export function getGameLook() {
  return gameLook;
}

/** Applies the settings to the page: `<html data-theme data-size data-face data-spacing data-width>`. */
export function applySettings(s = settings.get()) {
  const html = document.documentElement;
  const own = s.theme === "auto" ? gameLook : null;
  const theme = s.theme === "auto" ? own?.theme : s.theme;
  if (theme) html.dataset.theme = theme;
  else delete html.dataset.theme;
  if (own?.accent) html.style.setProperty("--accent", own.accent);
  else html.style.removeProperty("--accent");
  html.dataset.size = s.size;
  html.dataset.face = s.face;
  html.dataset.spacing = s.spacing;
  html.dataset.width = s.width;
}

// ------------------------------------------------------- your own games

export interface LocalGame {
  key: string;
  title: string;
  files: Files;
  added: number;
}

export const localGames = {
  list: (): LocalGame[] =>
    read<string[]>("local", [])
      .map((k) => read<LocalGame | null>(`local:${k}`, null))
      .filter((g): g is LocalGame => g !== null)
      .sort((a, b) => b.added - a.added),
  get: (key: string) => read<LocalGame | null>(`local:${key}`, null),
  /** Stores a game; false if there's no room for it. */
  put(game: LocalGame): boolean {
    if (!write(`local:${game.key}`, game)) return false;
    const keys = read<string[]>("local", []).filter((k) => k !== game.key);
    return write("local", [game.key, ...keys]);
  },
  remove(key: string) {
    remove(`local:${key}`);
    write("local", read<string[]>("local", []).filter((k) => k !== key));
  },
};

/**
 * How much of this browser's storage the site uses, and about how much it
 * may: browsers keep about 5 million characters for a site, two bytes each.
 */
export function storageUse(): { used: number; limit: number } {
  let chars = 0;
  try {
    for (let i = 0; i < localStorage.length; i++) {
      const k = localStorage.key(i) ?? "";
      chars += k.length + (localStorage.getItem(k)?.length ?? 0);
    }
  } catch {
    /* storage turned off: nothing used */
  }
  return { used: chars * 2, limit: 10_000_000 };
}

/** A short, stable key for a set of files. */
export function hashFiles(files: Files): string {
  let h = 2166136261;
  for (const [path, text] of Object.entries(files).sort(([a], [b]) => a.localeCompare(b))) {
    for (const s of [path, "\0", text, "\0"])
      for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 16777619) >>> 0;
  }
  return h.toString(36);
}

// ---------------------------------------------------------------- saves

export interface Slot {
  name: string;
  at: number;
  save: string;
}

/** A saved game as a downloadable file. */
export interface SaveFile {
  format: "eifgb-save/1";
  game: string;
  title: string;
  name: string;
  at: number;
  save: string;
}

/** Saves for one game: `game` is e.g. `example:campfire` or `local:<key>`. */
export function savesFor(game: string) {
  return {
    auto: () => read<string | null>(`auto:${game}`, null),
    setAuto: (save: string) => write(`auto:${game}`, save),
    clearAuto: () => remove(`auto:${game}`),
    slots: () => read<Slot[]>(`slots:${game}`, []).sort((a, b) => b.at - a.at),
    /** Adds (or replaces, by name) a slot. False if there's no room. */
    put(slot: Slot): boolean {
      const rest = read<Slot[]>(`slots:${game}`, []).filter((s) => s.name !== slot.name);
      return write(`slots:${game}`, [slot, ...rest]);
    },
    remove(name: string) {
      write(`slots:${game}`, read<Slot[]>(`slots:${game}`, []).filter((s) => s.name !== name));
    },
  };
}

// ------------------------------------------------------------ bookmarks

/** A moment in the story the player marked, to read again (or go back to). */
export interface Bookmark {
  name: string;
  at: number;
  room: string;
  turn: number;
  /** What was on the page: the command ("> look") and the reply, a line each. */
  text: string[];
  /** The game as it was then, to go back to. */
  save: string;
}

/** Bookmarks for one game, newest first. */
export function bookmarksFor(game: string) {
  return {
    list: () => read<Bookmark[]>(`bookmarks:${game}`, []).sort((a, b) => b.at - a.at),
    /** False if there's no room. */
    add: (b: Bookmark) => write(`bookmarks:${game}`, [b, ...read<Bookmark[]>(`bookmarks:${game}`, [])]),
    remove: (at: number) => write(`bookmarks:${game}`, read<Bookmark[]>(`bookmarks:${game}`, []).filter((b) => b.at !== at)),
    rename: (at: number, name: string) => write(`bookmarks:${game}`, read<Bookmark[]>(`bookmarks:${game}`, []).map((b) => (b.at === at ? { ...b, name } : b))),
  };
}

// ------------------------------------------------------------- the map

/** Whether the player's map panel is open; `null` until they've chosen. */
export const mapPanel = {
  open: (): boolean | null => read<boolean | null>("map-open", null),
  set: (open: boolean) => void write("map-open", open),
};

// ------------------------------------------------------------- endings

interface EndingsFound {
  found: string[];
  total: number;
}

/** The endings a player has reached in a game, kept in this browser. */
export function endingsFor(game: string) {
  const key = `endings:${game}`;
  return {
    get: (): EndingsFound => read<EndingsFound>(key, { found: [], total: 0 }),
    /** Notes an ending reached; `all` is every ending the game has. */
    record(id: string, all: string[]) {
      const known = read<EndingsFound>(key, { found: [], total: 0 }).found.filter((f) => all.includes(f));
      const found = known.includes(id) ? known : [...known, id];
      write(key, { found, total: all.length });
    },
  };
}

/** "You've found 2 of 3 endings.", or null before the first. */
export function endingsText(game: string): string | null {
  const { found, total } = endingsFor(game).get();
  if (!found.length) return null;
  if (total <= 1) return "You've found the ending.";
  if (found.length >= total) return total === 2 ? "You've found both endings." : `You've found all ${total} endings.`;
  return `You've found ${found.length} of ${total} endings.`;
}

// --------------------------------------------------- the editor's map

/** The editor's map theme: the site's colours, or a blueprint (white lines on blue). */
export type MapTheme = "site" | "blueprint";
export const mapTheme = {
  get: (): MapTheme => (read<string>("map-theme", "site") === "blueprint" ? "blueprint" : "site"),
  set: (t: MapTheme) => void write("map-theme", t),
};

/** Whether the editor's Getting started checklist is hidden, for every game in this browser. */
export const gettingStartedHidden = {
  get: (): boolean => read<boolean>("getting-started-hidden", false),
  set: (hidden: boolean) => void write("getting-started-hidden", hidden),
};
