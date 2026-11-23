// Find in game: every room, thing, character, conversation and script that
// mentions a word, with where it is, to go to.

import { parse } from "yaml";
import type { Files } from "../engine.ts";

export interface Found {
  /** For game.yaml: the dotted path ("rooms.attic.description"); for a script, its file. */
  path: string;
  /** A script file's line, from 1. */
  line?: number;
  /** What it's in, for people: "Room Attic", "scripts/doors.lua, line 4". */
  where: string;
  /** Which part: "description", "name", "on enter". */
  field: string;
  /** The text around the match, and where in it the match is. */
  text: string;
  at: number;
  length: number;
}

const KINDS: Record<string, string> = {
  rooms: "Room",
  objects: "Thing",
  characters: "Character",
  factions: "Side",
  endings: "Ending",
  timers: "Timer",
  hints: "Hint",
  goals: "Goal",
  choices: "Conversation",
  verbs: "Verb",
  vars: "Variable",
  aliases: "Word",
  messages: "Message",
};

/** At most this many results, so a common word doesn't flood the list. */
export const MAX_FOUND = 200;
/** How much text to show either side of a match. */
const AROUND = 40;

function excerpt(text: string, at: number, length: number): { text: string; at: number } {
  const flat = text.replace(/\s+/g, " ");
  // Whitespace runs collapse, so find the match again in the flattened text.
  const before = text.slice(0, at).replace(/\s+/g, " ");
  const start = Math.max(0, before.length - AROUND);
  const end = Math.min(flat.length, before.length + length + AROUND);
  const pre = start > 0 ? "…" : "";
  return { text: `${pre}${flat.slice(start, end)}${end < flat.length ? "…" : ""}`, at: pre.length + before.length - start };
}

/** Every place `query` appears, ignoring case: names and IDs too. */
export function findInGame(files: Files, query: string): Found[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  const out: Found[] = [];
  const add = (f: Omit<Found, "text" | "at" | "length">, text: string) => {
    // Once for each field, at its first match.
    const i = text.toLowerCase().indexOf(q);
    if (i < 0 || out.length >= MAX_FOUND) return;
    const e = excerpt(text, i, q.length);
    out.push({ ...f, text: e.text, at: e.at, length: q.length });
  };

  let doc: unknown = null;
  try {
    doc = parse(files["game.yaml"] ?? "");
  } catch {
    /* a file with a mistake: search its text line by line below */
  }
  if (doc && typeof doc === "object") {
    const top = doc as Record<string, unknown>;
    for (const [section, value] of Object.entries(top)) {
      const kind = KINDS[section];
      if (kind && value && typeof value === "object" && !Array.isArray(value)) {
        for (const [id, entry] of Object.entries(value as Record<string, unknown>)) {
          const name = entry && typeof entry === "object" && typeof (entry as Record<string, unknown>).name === "string" ? (entry as Record<string, string>).name : id;
          const where = `${kind} ${name}`;
          add({ path: `${section}.${id}`, where, field: "ID" }, id);
          walk(entry, [section, id], (path, text) => add({ path: path.join("."), where, field: fieldName(path.slice(2)) }, text));
        }
      } else {
        walk(value, [section], (path, text) => add({ path: path.join("."), where: section === "game" ? "The game" : `The ${section} section`, field: fieldName(path.slice(1)) }, text));
      }
    }
  } else {
    lines(files["game.yaml"] ?? "", "game.yaml", add);
  }
  for (const f of Object.keys(files).sort()) {
    if (f.endsWith(".lua") || /^walkthrough(-[^/]+)?\.txt$/.test(f)) lines(files[f], f, add);
  }
  return out;
}

function lines(text: string, file: string, add: (f: Omit<Found, "text" | "at" | "length">, text: string) => void) {
  text.split("\n").forEach((l, i) => add({ path: file, line: i + 1, where: `${file}, line ${i + 1}`, field: "" }, l));
}

/** Calls `fn` with every string under `v`, and its path. */
function walk(v: unknown, path: (string | number)[], fn: (path: (string | number)[], text: string) => void) {
  if (typeof v === "string") fn(path, v);
  else if (typeof v === "number" || typeof v === "boolean") fn(path, String(v));
  else if (Array.isArray(v)) v.forEach((x, i) => walk(x, [...path, i], fn));
  else if (v && typeof v === "object") for (const [k, x] of Object.entries(v)) walk(x, [...path, k], fn);
}

/** "description", "on enter", "exits north", "steps 2". */
function fieldName(path: (string | number)[]): string {
  return path.map((p) => (typeof p === "number" ? String(p + 1) : p.replace(/_/g, " "))).join(" ");
}
