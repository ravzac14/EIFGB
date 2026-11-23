// Possible typos in the game's own text: a word used once that's one slip
// from a word the game uses more often, or from one of its own names
// ("lantren" when the game says "lantern" everywhere else). There's no
// dictionary: it's the game against itself, so its names and made-up
// words are always allowed. The browser's own spell check underlines
// words in the editor's text boxes as well.

import { parse } from "yaml";

export interface Typo {
  word: string;
  /** What it's probably a slip for. */
  like: string;
  /** Where it is: a dotted path in game.yaml. */
  path: string;
}

/** Fields that are Lua, IDs or settings, not words players read. */
const NOT_PROSE = new Set(["do", "when", "on", "start", "in", "goto", "through", "to", "unlocks_with", "talk", "faction", "end", "kind", "traits", "state", "id", "version", "format", "scripts", "picture", "file", "started_by", "stopped_by", "revealed_by", "heard_in", "behavior", "patterns", "at", "every", "after", "presentation", "settings", "clock", "vars"]);

/** Edit distance, with a swap of neighbours as one slip. */
function distance(a: string, b: string): number {
  const d: number[][] = Array.from({ length: a.length + 1 }, (_, i) => [i, ...Array(b.length).fill(0)]);
  for (let j = 1; j <= b.length; j++) d[0][j] = j;
  for (let i = 1; i <= a.length; i++)
    for (let j = 1; j <= b.length; j++) {
      d[i][j] = Math.min(d[i - 1][j] + 1, d[i][j - 1] + 1, d[i - 1][j - 1] + (a[i - 1] === b[j - 1] ? 0 : 1));
      if (i > 1 && j > 1 && a[i - 1] === b[j - 2] && a[i - 2] === b[j - 1]) d[i][j] = Math.min(d[i][j], d[i - 2][j - 2] + 1);
    }
  return d[a.length][b.length];
}

/** Whether two words are forms of one word: key and keys, glint and glints, open and opened. */
function sameWord(a: string, b: string): boolean {
  const [s, l] = a.length < b.length ? [a, b] : [b, a];
  return ["s", "es", "ed", "d", "ing", "er", "ly", "'s", "y"].some((end) => l === s + end) || (s.endsWith("e") && l === `${s.slice(0, -1)}ing`);
}

/** Every piece of prose in the file, with its path. */
function prose(v: unknown, path: (string | number)[], out: [string, string][]) {
  const key = path[path.length - 1];
  if (typeof key === "string" && NOT_PROSE.has(key)) return;
  if (typeof v === "string") out.push([path.join("."), v.replace(/\$\{[^}]*\}/g, " ")]);
  else if (Array.isArray(v)) v.forEach((x, i) => prose(x, [...path, i], out));
  else if (v && typeof v === "object") for (const [k, x] of Object.entries(v)) prose(x, [...path, k], out);
}

/** Possible typos in game.yaml's prose, most likely first (at most `limit`). */
export function typos(yaml: string, limit = 30): Typo[] {
  let doc: unknown;
  try {
    doc = parse(yaml);
  } catch {
    return [];
  }
  if (!doc || typeof doc !== "object") return [];
  const texts: [string, string][] = [];
  prose(doc, [], texts);
  // The game's own names: entity IDs and their words, always allowed.
  const names = new Set<string>();
  for (const section of ["rooms", "objects", "characters", "endings", "choices", "factions"]) {
    for (const [id, e] of Object.entries(((doc as Record<string, unknown>)[section] ?? {}) as Record<string, unknown>)) {
      for (const w of id.split("_")) names.add(w);
      const entry = (e ?? {}) as Record<string, unknown>;
      for (const f of ["name", "synonyms", "adjectives"]) {
        const v = entry[f];
        for (const s of Array.isArray(v) ? v : typeof v === "string" ? [v] : []) for (const w of String(s).toLowerCase().split(/[^a-z']+/)) if (w) names.add(w);
      }
    }
  }
  const count = new Map<string, number>();
  const where = new Map<string, string>();
  for (const [path, text] of texts) {
    for (const m of text.matchAll(/[A-Za-z][a-z']+/g)) {
      const w = m[0].toLowerCase().replace(/'s$/, "");
      count.set(w, (count.get(w) ?? 0) + 1);
      if (!where.has(w)) where.set(w, path);
    }
  }
  const common = [...count].filter(([w, n]) => n >= 2 && w.length >= 4).map(([w]) => w);
  const known = [...new Set([...common, ...[...names].filter((w) => w.length >= 4)])];
  const out: Typo[] = [];
  for (const [w, n] of count) {
    if (n !== 1 || w.length < 5 || names.has(w)) continue;
    const like = known.find((k) => k !== w && Math.abs(k.length - w.length) <= 1 && k[0] === w[0] && distance(k, w) === 1 && !sameWord(k, w));
    if (like) out.push({ word: w, like, path: where.get(w)! });
    if (out.length >= limit) break;
  }
  return out;
}
