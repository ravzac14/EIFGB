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

export type Theme = "auto" | "light" | "dark" | "sepia" | "terminal" | "amber" | "contrast";
export type Size = "small" | "medium" | "large";
export type Face = "serif" | "sans" | "legible";
export type Spacing = "normal" | "relaxed";
export type Width = "narrow" | "medium" | "wide";
export interface Settings {
  theme: Theme;
  size: Size;
  face: Face;
  spacing: Spacing;
  width: Width;
}
const DEFAULTS: Settings = { theme: "auto", size: "medium", face: "serif", spacing: "normal", width: "medium" };

export const settings = {
  get: (): Settings => ({ ...DEFAULTS, ...read<Partial<Settings>>("settings", {}) }),
  set: (s: Settings) => write("settings", s),
};

/** Applies the settings to the page: `<html data-theme data-size data-face data-spacing data-width>`. */
export function applySettings(s = settings.get()) {
  const html = document.documentElement;
  if (s.theme === "auto") delete html.dataset.theme;
  else html.dataset.theme = s.theme;
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

// ------------------------------------------------------------- the map

/** Whether the player's map panel is open; `null` until they've chosen. */
export const mapPanel = {
  open: (): boolean | null => read<boolean | null>("map-open", null),
  set: (open: boolean) => void write("map-open", open),
};
