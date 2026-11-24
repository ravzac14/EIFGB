// How each character sounds: their lines side by side (what they say in
// conversations, chatter, favours and memories), a few things about how
// they talk, and two hints: characters who sound alike, and one whose
// voice changes from their early lines to their later ones. Only a guess
// from the words, to send the author back to read them.

import { parse } from "yaml";

export interface Voice {
  id: string;
  name: string;
  /** Their lines, in the file's order. */
  lines: string[];
  words: number;
  /** Words a sentence, on average. */
  perSentence: number;
  /** Of their sentences: questions, and exclamations (0 to 1). */
  questions: number;
  exclaims: number;
  /** Of their words, how many are contractions (don't, I'm). */
  contractions: number;
  /** Words they use that others don't, most telling first. */
  own: string[];
}

export interface VoiceReport {
  voices: Voice[];
  /** Pairs who sound alike: how alike, 0 to 1. */
  alike: { a: string; b: string; score: number }[];
  /** Characters whose later lines sound unlike their earlier ones. */
  shifts: { id: string; score: number }[];
}

/** Too few words to say anything about. */
const ENOUGH = 25;
const COMMON = new Set("the a an and or but of to in on at for with is are was were be been it its this that i you he she we they me him her us them my your his our their not no so if as by from do does did have has had will would can could just there here what who all".split(" "));

/** The spoken part of a line: what's in quotes, or all of it if nothing is. */
function spoken(text: string): string {
  const quoted = [...text.matchAll(/[“"]([^”"]+)[”"]/g)].map((m) => m[1]);
  return (quoted.length ? quoted.join(" ") : text).replace(/\$\{[^}]*\}/g, " ");
}

const wordsOf = (s: string) => s.toLowerCase().match(/[a-z]+(?:'[a-z]+)?/g) ?? [];

function counts(lines: string[]): Map<string, number> {
  const m = new Map<string, number>();
  for (const l of lines) for (const w of wordsOf(l)) m.set(w, (m.get(w) ?? 0) + 1);
  return m;
}

/** How alike two word counts are: the cosine of the angle between them. */
function cosine(a: Map<string, number>, b: Map<string, number>): number {
  let dot = 0;
  let na = 0;
  let nb = 0;
  for (const [w, x] of a) {
    na += x * x;
    dot += x * (b.get(w) ?? 0);
  }
  for (const x of b.values()) nb += x * x;
  return na && nb ? dot / Math.sqrt(na * nb) : 0;
}

/** A style fingerprint: how often they use the small words, and how they end sentences. */
function style(lines: string[]): Map<string, number> {
  const m = new Map<string, number>();
  for (const [w, n] of counts(lines)) if (COMMON.has(w) || w.includes("'")) m.set(w, n);
  const text = lines.join(" ");
  m.set("?", (text.match(/\?/g) ?? []).length * 2);
  m.set("!", (text.match(/!/g) ?? []).length * 2);
  m.set("…", (text.match(/…|\.\.\./g) ?? []).length * 2);
  return m;
}

/** Every string under `v`. */
function strings(v: unknown, out: string[] = []): string[] {
  if (typeof v === "string") out.push(v);
  else if (Array.isArray(v)) v.forEach((x) => strings(x, out));
  else if (v && typeof v === "object") for (const x of Object.values(v)) strings(x, out);
  return out;
}

/** Each character's lines, and what they suggest. */
export function voices(yaml: string): VoiceReport {
  let doc: Record<string, unknown>;
  try {
    doc = (parse(yaml) ?? {}) as Record<string, unknown>;
  } catch {
    return { voices: [], alike: [], shifts: [] };
  }
  const chars = (doc.characters ?? {}) as Record<string, Record<string, unknown>>;
  const choices = (doc.choices ?? {}) as Record<string, { nodes?: Record<string, { text?: unknown; options?: { say?: unknown }[] }> }>;
  const out: Voice[] = [];
  for (const [id, c] of Object.entries(chars)) {
    if (!c || typeof c !== "object") continue;
    const lines: string[] = [];
    const talk = typeof c.talk === "string" ? choices[c.talk] : undefined;
    for (const n of Object.values(talk?.nodes ?? {})) {
      lines.push(...strings(n?.text));
      for (const o of n?.options ?? []) lines.push(...strings(o?.say));
    }
    for (const ch of (Array.isArray(c.chatter) ? c.chatter : []) as unknown[]) lines.push(...strings(typeof ch === "object" && ch ? (ch as { say?: unknown }).say : ch));
    lines.push(...strings(c.remembers));
    const asks = c.asks as Record<string, unknown> | undefined;
    if (asks) lines.push(...strings([asks.say, asks.thanks, asks.broken]));
    const said = lines.map(spoken).map((l) => l.trim()).filter(Boolean);
    if (!said.length) continue;
    const ws = said.flatMap(wordsOf);
    const sentences = said.flatMap((l) => l.split(/(?<=[.!?…])\s+/)).filter((s) => wordsOf(s).length);
    out.push({
      id,
      name: typeof c.name === "string" ? c.name : id,
      lines: said,
      words: ws.length,
      perSentence: sentences.length ? Math.round((ws.length / sentences.length) * 10) / 10 : 0,
      questions: sentences.length ? sentences.filter((s) => s.trim().endsWith("?")).length / sentences.length : 0,
      exclaims: sentences.length ? sentences.filter((s) => s.trim().endsWith("!")).length / sentences.length : 0,
      contractions: ws.length ? ws.filter((w) => w.includes("'")).length / ws.length : 0,
      own: [],
    });
  }
  // Their own words: often theirs, rarely anyone else's.
  const all = out.map((v) => counts(v.lines));
  out.forEach((v, i) => {
    const mine = all[i];
    v.own = [...mine]
      .filter(([w, n]) => !COMMON.has(w) && w.length > 3 && n >= 2 && all.every((o, j) => j === i || !o.has(w)))
      .sort((a, b) => b[1] - a[1])
      .slice(0, 5)
      .map(([w]) => w);
  });
  const alike: VoiceReport["alike"] = [];
  const styles = out.map((v) => style(v.lines));
  for (let i = 0; i < out.length; i++)
    for (let j = i + 1; j < out.length; j++) {
      if (out[i].words < ENOUGH || out[j].words < ENOUGH) continue;
      const score = cosine(styles[i], styles[j]);
      if (score >= 0.93) alike.push({ a: out[i].id, b: out[j].id, score: Math.round(score * 100) / 100 });
    }
  const shifts: VoiceReport["shifts"] = [];
  for (const v of out) {
    if (v.words < ENOUGH * 2 || v.lines.length < 4) continue;
    const half = Math.floor(v.lines.length / 2);
    const score = cosine(style(v.lines.slice(0, half)), style(v.lines.slice(half)));
    if (score < 0.6) shifts.push({ id: v.id, score: Math.round(score * 100) / 100 });
  }
  return { voices: out, alike: alike.sort((a, b) => b.score - a.score), shifts };
}
