// Undo history: a few words for each change ("Added the room Attic",
// "Moved rooms on the map"), worked out by comparing the files before and
// after it, so every edit gets a label without each one naming itself.

import { parse } from "yaml";
import type { Files } from "../engine.ts";

/** Sections whose entries are things the author names and works on one at a time. */
const ENTRIES: Record<string, string> = {
  rooms: "room",
  objects: "thing",
  characters: "character",
  factions: "side",
  endings: "ending",
  timers: "timer",
  hints: "hint",
  goals: "goal",
  choices: "conversation",
  verbs: "verb",
  vars: "variable",
  aliases: "word",
};

/** Other parts of the file, by what an author would call them. */
const PARTS: Record<string, string> = {
  game: "the game's details",
  player: "the player",
  settings: "the settings",
  presentation: "the game's look",
  clock: "the clock",
  on: "the game's scripts",
  scripts: "the script files",
  messages: "the messages",
  weather: "the weather",
};

type Doc = Record<string, unknown>;

function read(text: string | undefined): Doc {
  try {
    const d = parse(text ?? "");
    return d && typeof d === "object" ? (d as Doc) : {};
  } catch {
    return {};
  }
}

const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

function nameOf(entry: unknown, id: string): string {
  if (entry && typeof entry === "object") {
    const e = entry as Record<string, unknown>;
    if (typeof e.name === "string" && e.name.trim()) return e.name;
    if (typeof e.text === "string" && e.text.trim()) return e.text.length > 30 ? `${e.text.slice(0, 28)}…` : e.text;
  }
  return id;
}

/** What changed in game.yaml, a phrase per change. */
function gameChanges(a: Doc, b: Doc): string[] {
  const out: string[] = [];
  const keys = new Set([...Object.keys(a), ...Object.keys(b)]);
  for (const k of keys) {
    if (same(a[k], b[k])) continue;
    const kind = ENTRIES[k];
    if (!kind) {
      const part = PARTS[k] ?? `\`${k}\``;
      out.push(a[k] === undefined ? `Added ${part}` : b[k] === undefined ? `Removed ${part}` : `Changed ${part}`);
      continue;
    }
    const was = (a[k] ?? {}) as Doc;
    const now = (b[k] ?? {}) as Doc;
    const added = Object.keys(now).filter((id) => !(id in was));
    const removed = Object.keys(was).filter((id) => !(id in now));
    // A rename: one gone, one new, in the same change.
    if (added.length === 1 && removed.length === 1) {
      const [from, to] = [nameOf(was[removed[0]], removed[0]), nameOf(now[added[0]], added[0])];
      out.push(from === to ? `Renamed the ${kind} ${from}'s ID to ${added[0]}` : `Renamed the ${kind} ${from} to ${to}`);
    } else {
      for (const id of added) out.push(`Added the ${kind} ${nameOf(now[id], id)}`);
      for (const id of removed) out.push(`Deleted the ${kind} ${nameOf(was[id], id)}`);
    }
    for (const id of Object.keys(now)) {
      if (id in was && !same(was[id], now[id])) out.push(`Changed the ${kind} ${nameOf(now[id], id)}`);
    }
  }
  return out;
}

/** A short label for the change from `before` to `after`. */
export function describeChange(before: Files, after: Files): string {
  const out = gameChanges(read(before["game.yaml"]), read(after["game.yaml"]));
  const files = new Set([...Object.keys(before), ...Object.keys(after)]);
  for (const f of [...files].sort()) {
    if (f === "game.yaml" || before[f] === after[f]) continue;
    if (f === "map.yaml") {
      // Only worth saying when nothing in the game changed: a new room gets a place too.
      if (!out.length) out.push("Moved rooms on the map");
    } else if (f === "walkthrough.txt") out.push(after[f] === undefined ? "Deleted the walkthrough" : "Saved the walkthrough");
    else if (f.startsWith("pictures/")) out.push(after[f] === undefined ? `Removed the picture ${f.slice(9)}` : `Added the picture ${f.slice(9)}`);
    else out.push(before[f] === undefined ? `Added ${f}` : after[f] === undefined ? `Deleted ${f}` : `Edited ${f}`);
  }
  // Renaming reaches into exits and scripts: the rename is what the author did.
  const rename = out.find((o) => o.startsWith("Renamed"));
  if (rename) return rename;
  if (!out.length) return "A change";
  if (out.length === 2) return `${out[0]}, and ${out[1].charAt(0).toLowerCase()}${out[1].slice(1)}`;
  if (out.length === 1) return out[0];
  return `${out[0]}, and ${out.length - 1} more changes`;
}

/** "just now", "3 minutes ago", "at 14:05". */
export function when(at: number, now = Date.now()): string {
  const s = Math.round((now - at) / 1000);
  if (s < 45) return "just now";
  const m = Math.round(s / 60);
  if (m < 60) return `${m} minute${m === 1 ? "" : "s"} ago`;
  const d = new Date(at);
  return `at ${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}
