// Possible typos in the game's own text. With the dictionary loaded (a word
// list kept with the site, fetched once: nothing about the game is sent
// anywhere), a word that's neither in it nor one of the game's names, and
// is one slip from a word that is ("lantren", "lantern"), however often it's
// used. Without it, the game against itself: a word used once that's one
// slip from a word the game uses more often, or from one of its names.
// Either way the game's names and made-up words are allowed, and a word
// with nothing close to it isn't a typo, just a word we don't know. The
// browser's own spell check underlines words in the text boxes as well.

import { parse } from "yaml";

export interface Typo {
  word: string;
  /** What it's probably a slip for. */
  like: string;
  /** Where it is (the first time): a dotted path in game.yaml. */
  path: string;
  /** How many times it's used, if more than once. */
  times?: number;
}

/** The dictionary: each word, with how common it is (0 the commonest). */
let dictionary: Map<string, number> | null = null;
let loading: Promise<boolean> | null = null;

/** Uses these words as the dictionary, commonest first. */
export function useDictionary(words: string[] | null) {
  dictionary = words ? new Map(words.map((w, i) => [w, i])) : null;
  last = null;
}

/** Whether the dictionary is ready. */
export const hasDictionary = () => dictionary !== null;

/** Fetches the dictionary (once); true when it's ready. */
export function loadDictionary(url = "./words.txt"): Promise<boolean> {
  loading ??= fetch(url)
    .then((r) => (r.ok ? r.text() : Promise.reject(new Error(String(r.status)))))
    .then((t) => {
      useDictionary(t.split("\n").filter(Boolean));
      return true;
    })
    // Offline, say: the check works without it, as before, until next time.
    .catch(() => false);
  return loading;
}

/** Every word one slip from `w`: a letter left out, added, changed or two swapped. */
function slips(w: string): string[] {
  const abc = "abcdefghijklmnopqrstuvwxyz";
  const out: string[] = [];
  for (let i = 0; i <= w.length; i++) {
    const [a, b] = [w.slice(0, i), w.slice(i)];
    if (b) out.push(a + b.slice(1));
    if (b.length > 1) out.push(a + b[1] + b[0] + b.slice(2));
    for (const c of abc) {
      if (b && c !== b[0]) out.push(a + c + b.slice(1));
      out.push(a + c + b);
    }
  }
  return out;
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

/** The last answer, for the same text: it's asked after every edit. */
let last: { yaml: string; limit: number; out: Typo[] } | null = null;


/** Possible typos in game.yaml's prose, most likely first (at most `limit`). */
export function typos(yaml: string, limit = 30): Typo[] {
  if (last?.yaml === yaml && last.limit === limit) return last.out;
  const out = findTypos(yaml, limit);
  last = { yaml, limit, out };
  return out;
}

function findTypos(yaml: string, limit: number): Typo[] {
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
  if (dictionary) {
    const dict = dictionary;
    // Forms of the game's names are names too: lanterns, Elsbeth's.
    const isName = (w: string) => names.has(w) || [...names].some((n) => n.length >= 3 && sameWord(n, w));
    const gameWords = new Set(known);
    for (const [w, n] of count) {
      // Contractions: the list has none, so "won't" is "won", "she'd" "she".
      if (w.length < 4 || dict.has(w) || dict.has(w.split("'")[0]) || isName(w)) continue;
      // The likeliest word it's a slip for: one of the game's; else two
      // letters swapped (the commonest slip); else the commonest word.
      const near = slips(w).filter((k) => k !== w && k[0] === w[0] && (gameWords.has(k) || dict.has(k)) && !sameWord(k, w));
      const swapped = (k: string) => k.length === w.length && [...k].sort().join() === [...w].sort().join();
      const like = near.find((k) => gameWords.has(k)) ?? near.sort((a, b) => Number(swapped(b)) - Number(swapped(a)) || dict.get(a)! - dict.get(b)!)[0];
      if (like) out.push({ word: w, like, path: where.get(w)!, ...(n > 1 ? { times: n } : {}) });
      if (out.length >= limit) break;
    }
    return out;
  }
  for (const [w, n] of count) {
    if (n !== 1 || w.length < 5 || names.has(w)) continue;
    const like = known.find((k) => k !== w && Math.abs(k.length - w.length) <= 1 && k[0] === w[0] && distance(k, w) === 1 && !sameWord(k, w));
    if (like) out.push({ word: w, like, path: where.get(w)! });
    if (out.length >= limit) break;
  }
  return out;
}
