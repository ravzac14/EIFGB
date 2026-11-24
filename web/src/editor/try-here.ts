// "What could a player try here?": for a room, what a curious player types
// at each thing and person in it (PULL THE LEVER, SMELL THE ROSES), and
// whether the game answers with something of its own or only the stock
// reply ("Nothing obvious happens."). Read from the game, not played: a
// script that only answers sometimes still counts as an answer.

import type { Model } from "./model.ts";

export interface Try {
  /** What the player types: "PULL LEVER". */
  command: string;
  /** Whether the game has something of its own to say or do. */
  does: boolean;
  /** Why it does: "on: pull", "it's pushable". */
  why?: string;
}

export interface TriesFor {
  id: string;
  name: string;
  tries: Try[];
}

/** What a player pokes at, by verb ID, with the words they'd type. */
const THINGS: [string, string][] = [
  ["examine", "EXAMINE"],
  ["push", "PUSH"],
  ["pull", "PULL"],
  ["twist", "TURN"],
  ["touch", "TOUCH"],
  ["smell", "SMELL"],
  ["listen", "LISTEN TO"],
  ["search", "SEARCH"],
];
const PEOPLE: [string, string][] = [
  ["examine", "EXAMINE"],
  ["talk", "TALK TO"],
  ["give", "GIVE"],
  ["attack", "ATTACK"],
];

const asList = (v: unknown): string[] => (Array.isArray(v) ? v.map(String) : typeof v === "string" ? [v] : []);
const has = (m: Model, path: string[]) => {
  const v = m.get(path);
  return v !== undefined && v !== null && v !== "" && !(Array.isArray(v) && !v.length);
};

/** Verbs that something answers, from its handlers and `revealed_by`s that name it. */
function answered(m: Model, section: string, id: string): Map<string, string> {
  const out = new Map<string, string>();
  for (const h of m.handlers([section, id])) out.set(h.event.replace(/^(before|after)_/, ""), `on: ${h.event}`);
  // Hidden things found by doing something to this one.
  for (const other of m.ids("objects")) {
    if (m.get(["objects", other, "in"]) !== id) continue;
    for (const v of asList(m.get(["objects", other, "revealed_by"]))) out.set(v.split(/\s+/)[0], `reveals ${other}`);
  }
  return out;
}

/** For each thing (not hidden, not a door) and person in `room`. */
export function triesHere(m: Model, room: string): TriesFor[] {
  const out: TriesFor[] = [];
  for (const t of m.things()) {
    if (t.in !== room || t.hidden) continue;
    const p = ["objects", t.id];
    const by = answered(m, "objects", t.id);
    const traits = t.traits;
    const own = (verb: string): string | undefined => {
      if (by.has(verb)) return by.get(verb);
      if (verb === "examine" && t.description) return "a description";
      if (verb === "push" && traits.includes("pushable")) return "it's pushable";
      if (verb === "smell" && has(m, [...p, "smell"])) return "a smell";
      if (verb === "listen" && has(m, [...p, "sound"])) return "a sound";
      if (verb === "search" && (traits.includes("container") || traits.includes("supporter"))) return "things can be in it";
      return undefined;
    };
    const tries: Try[] = THINGS.map(([verb, word]) => {
      const why = own(verb);
      return { command: `${word} ${t.name.toUpperCase()}`, does: !!why, ...(why ? { why } : {}) };
    });
    // Scenery: TAKE is the first thing people try.
    if (traits.includes("scenery")) tries.splice(1, 0, { command: `TAKE ${t.name.toUpperCase()}`, does: by.has("take"), ...(by.has("take") ? { why: by.get("take") } : {}) });
    // Its own words: what else the game answers.
    for (const [verb, why] of by) if (!THINGS.some(([v]) => v === verb) && verb !== "take") tries.push({ command: `${verb.replace(/_/g, " ").toUpperCase()} ${t.name.toUpperCase()}`, does: true, why });
    out.push({ id: t.id, name: t.name, tries });
  }
  for (const c of m.peopleIn(room)) {
    const p = ["characters", c.id];
    const by = answered(m, "characters", c.id);
    const own = (verb: string): string | undefined => {
      if (by.has(verb)) return by.get(verb);
      if (verb === "examine" && has(m, [...p, "description"])) return "a description";
      if (verb === "talk" && has(m, [...p, "talk"])) return "a conversation";
      if (verb === "give" && (has(m, [...p, "likes"]) || has(m, [...p, "tame_with"]) || has(m, [...p, "bribe"]))) return "likes something";
      if (verb === "attack" && has(m, [...p, "health"])) return "can be hurt";
      return undefined;
    };
    const name = String(m.get([...p, "name"]) ?? c.id);
    const tries = PEOPLE.map(([verb, word]): Try => {
      const why = own(verb);
      const command = verb === "give" ? `GIVE SOMETHING TO ${name.toUpperCase()}` : `${word} ${name.toUpperCase()}`;
      return { command, does: !!why, ...(why ? { why } : {}) };
    });
    out.push({ id: c.id, name, tries });
  }
  return out;
}

/** How much of what a player might try is answered. */
export function answeredShare(rows: TriesFor[]): { does: number; all: number } {
  const all = rows.flatMap((r) => r.tries);
  return { does: all.filter((t) => t.does).length, all: all.length };
}
