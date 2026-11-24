// The prose check: the game's descriptions read together. Words leaned on
// in description after description, descriptions that run long, ones that
// are hard going, and rooms that all begin the same way. Only pointers,
// to send the author back to read.

import { parse } from "yaml";

export interface Passage {
  kind: "room" | "thing" | "character";
  id: string;
  name: string;
  text: string;
  words: number;
  /** How hard it is to read: a school grade (Flesch–Kincaid). */
  grade: number;
}

export interface ProseReport {
  passages: Passage[];
  /** Words used in many descriptions: how often, and where. */
  overused: { word: string; times: number; where: { kind: Passage["kind"]; id: string; name: string }[] }[];
  /** Longer than LONG words. */
  long: Passage[];
  /** The hardest to read, grade HARD and up. */
  hard: Passage[];
  /** Room descriptions that begin with the same words. */
  openings: { start: string; rooms: { id: string; name: string }[] }[];
  /** The average grade, by words. */
  grade: number;
}

/** Words past this, a description is long for a screen. */
export const LONG = 120;
/** A grade this high is hard going. */
export const HARD = 12;

const COMMON = new Set(
  "the a an and or but of to in on at for with is are was were be been it its this that you your he she we they me him her us them my his our their not no so if as by from do does did have has had will would can could just there here what who all into out up down over under some one two very more most than then there's it's you're you've isn't aren't".split(" "),
);

/** Every string under `v`. */
function strings(v: unknown, out: string[] = []): string[] {
  if (typeof v === "string") out.push(v);
  else if (Array.isArray(v)) v.forEach((x) => strings(x, out));
  else if (v && typeof v === "object") for (const x of Object.values(v)) strings(x, out);
  return out;
}

/** The words of a text, without its ${…} code. */
const clean = (s: string) => s.replace(/\$\{[^}]*\}/g, " ").replace(/\s+/g, " ").trim();
const wordsOf = (s: string) => s.toLowerCase().match(/[a-z]+(?:'[a-z]+)?/g) ?? [];

/** A rough count of a word's syllables. */
function syllables(w: string): number {
  const s = w.toLowerCase().replace(/'/g, "").replace(/(?:[^laeiouy]es|ed|[^laeiouy]e)$/, "").replace(/^y/, "");
  return Math.max(1, (s.match(/[aeiouy]{1,2}/g) ?? []).length);
}

/** Flesch–Kincaid grade: longer sentences and longer words read harder. */
export function grade(text: string): number {
  const ws = wordsOf(text);
  if (!ws.length) return 0;
  const sentences = Math.max(1, (text.match(/[.!?…]+(\s|$)/g) ?? []).length);
  const syl = ws.reduce((n, w) => n + syllables(w), 0);
  return Math.max(0, Math.round((0.39 * (ws.length / sentences) + 11.8 * (syl / ws.length) - 15.59) * 10) / 10);
}

/** The game's descriptions, and what reading them together shows. */
export function prose(yaml: string): ProseReport {
  let doc: Record<string, unknown>;
  try {
    doc = (parse(yaml) ?? {}) as Record<string, unknown>;
  } catch {
    return { passages: [], overused: [], long: [], hard: [], openings: [], grade: 0 };
  }
  const passages: Passage[] = [];
  const names = new Set<string>();
  for (const [section, kind] of [["rooms", "room"], ["objects", "thing"], ["characters", "character"]] as const) {
    for (const [id, e] of Object.entries((doc[section] ?? {}) as Record<string, Record<string, unknown>>)) {
      if (!e || typeof e !== "object") continue;
      const name = typeof e.name === "string" ? e.name : id;
      for (const w of wordsOf(name)) names.add(w);
      for (const w of strings(e.synonyms)) for (const x of wordsOf(w)) names.add(x);
      const text = clean(strings(e.description).join(" "));
      if (!wordsOf(text).length) continue;
      passages.push({ kind, id, name, text, words: wordsOf(text).length, grade: grade(text) });
    }
  }
  // Words leaned on: in three or more descriptions, four or more times.
  const seen = new Map<string, { times: number; where: Passage[] }>();
  for (const p of passages) {
    const counted = new Set<string>();
    for (const w of wordsOf(p.text)) {
      if (w.length < 4 || COMMON.has(w) || names.has(w) || names.has(w.replace(/s$/, ""))) continue;
      const e = seen.get(w) ?? { times: 0, where: [] };
      e.times++;
      if (!counted.has(w)) {
        e.where.push(p);
        counted.add(w);
      }
      seen.set(w, e);
    }
  }
  const overused = [...seen]
    .filter(([, e]) => e.where.length >= 3 && e.times >= 4)
    .sort((a, b) => b[1].times - a[1].times)
    .slice(0, 8)
    .map(([word, e]) => ({ word, times: e.times, where: e.where.map(({ kind, id, name }) => ({ kind, id, name })) }));
  // Rooms that begin alike: the same first three words, three times or more.
  const starts = new Map<string, { id: string; name: string }[]>();
  for (const p of passages.filter((x) => x.kind === "room")) {
    const first = p.text.split(/\s+/).slice(0, 3).join(" ").replace(/[,.;:!?]+$/, "");
    if (wordsOf(first).length < 3) continue;
    const key = first.toLowerCase();
    starts.set(key, [...(starts.get(key) ?? []), { id: p.id, name: p.name }]);
  }
  const openings = [...starts].filter(([, r]) => r.length >= 3).map(([start, rooms]) => ({ start: start[0].toUpperCase() + start.slice(1), rooms }));
  const total = passages.reduce((n, p) => n + p.words, 0);
  return {
    passages,
    overused,
    long: passages.filter((p) => p.words > LONG).sort((a, b) => b.words - a.words),
    hard: passages.filter((p) => p.grade >= HARD && p.words >= 12).sort((a, b) => b.grade - a.grade).slice(0, 5),
    openings,
    grade: total ? Math.round((passages.reduce((n, p) => n + p.grade * p.words, 0) / total) * 10) / 10 : 0,
  };
}
