// Words in descriptions: under a room's, thing's or character's
// description, the text again with the words players can already use
// highlighted (names and synonyms of what's here), and the nouns it
// mentions that aren't anything yet, each one click from being scenery.
// Players try what they read: "a narrow path climbs north" means someone
// will type EXAMINE PATH.

import type { Model } from "./model.ts";
import { button, el, type InspectorContext } from "./ui.ts";

export interface Part {
  text: string;
  /** `known`: a word for something here; `new`: a noun that isn't anything yet. */
  kind: "known" | "new" | "plain";
  /** For a known word: what it names. */
  what?: string;
}

/** Words that come before a noun. */
const DETERMINERS = new Set(["a", "an", "the", "some", "its", "his", "her", "their", "this", "that", "these", "those", "my", "your", "our", "another", "each", "every", "no"]);
/** Words that end a noun phrase. */
const STOP = new Set([
  "and", "or", "but", "of", "with", "in", "on", "at", "to", "from", "by", "for", "into", "onto", "over", "under", "behind", "through", "across", "beside", "near",
  "is", "are", "was", "were", "be", "been", "has", "have", "had", "it", "that", "which", "who", "where", "when", "as", "than", "then", "so", "if", "not",
  "nearby", "here", "there", "away", "above", "below", "ahead", "again", "too", "still", "now", "very", "off", "back", "around", "apart", "together",
  "would", "could", "should", "can", "will", "may", "might", "must", "shall", "does", "did", "do",
]);
/** Words that often follow a verb as an adverb: "starts here", "hangs open". */
const AFTER_VERB_ADVERB = new Set(["here", "there", "away", "nearby", "above", "below", "ahead", "again", "still", "off", "back", "around", "apart", "together", "open", "shut", "slowly", "quietly"]);
/** Words that often follow a verb: "climbs north", "shivers in", "hang by". */
const AFTER_VERB = new Set(["in", "on", "at", "to", "into", "onto", "over", "under", "through", "across", "beside", "near", "by", "from", "toward", "towards", "around", "along", "past", "against", "about", "off", "out", "up", "down"]);
/** Nouns too vague to be things. */
const VAGUE = new Set(["mile", "miles", "inch", "inches", "hour", "hours", "minute", "minutes", "day", "days", "year", "years", "way", "ways", "side", "sides", "top", "bottom", "edge", "end", "rest", "moment", "time", "place", "thing", "things", "bit", "kind", "sort", "lot", "air", "distance", "while", "middle", "direction", "smell", "sound", "light", "dark", "darkness", "silence", "corner"]);
const DIRECTIONS = new Set(["north", "south", "east", "west", "northeast", "northwest", "southeast", "southwest", "up", "down", "in", "out", "inside", "outside"]);

const singular = (w: string) => (w.endsWith("ies") ? `${w.slice(0, -3)}y` : w.endsWith("es") && /(s|x|ch|sh)es$/.test(w) ? w.slice(0, -2) : w.endsWith("s") && !w.endsWith("ss") ? w.slice(0, -1) : w);

/**
 * Reads a description: which words name something (`known`, word → what),
 * and which nouns don't (`skip`: words never to suggest, like the room's
 * own name). `${…}` parts are left as they are.
 */
export function readDescription(text: string, known: Map<string, string>, skip: Set<string> = new Set()): { parts: Part[]; nouns: string[] } {
  // Template parts aren't words to look at: mark where they are.
  const masked = text.replace(/\$\{[^}]*\}/g, (m) => "\u0000".repeat(m.length));
  const tokens = [...masked.matchAll(/[A-Za-z][A-Za-z'-]*|[^A-Za-z]+/g)].map((m) => ({ at: m.index ?? 0, len: m[0].length }));
  const words = tokens.map((t) => text.slice(t.at, t.at + t.len));
  const lower = words.map((w) => w.toLowerCase());
  const isWord = (i: number) => /^[A-Za-z]/.test(masked.slice(tokens[i].at, tokens[i].at + tokens[i].len));
  const knownAs = (w: string) => known.get(w) ?? known.get(singular(w));
  // The noun of each "the narrow path": the last word before a stop.
  const heads = new Set<number>();
  for (let i = 0; i < words.length; i++) {
    if (!isWord(i) || !DETERMINERS.has(lower[i])) continue;
    const phrase: number[] = [];
    let next = "";
    for (let j = i + 1; j < words.length && phrase.length < 3; j++) {
      if (!isWord(j)) {
        // Only spaces carry the phrase on.
        if (/^[ ]+$/.test(words[j])) continue;
        break;
      }
      if (STOP.has(lower[j]) || DETERMINERS.has(lower[j]) || DIRECTIONS.has(lower[j])) {
        next = lower[j];
        break;
      }
      phrase.push(j);
      // A word for something here is the noun ("the tide pool shivers").
      if (knownAs(lower[j])) break;
    }
    if (!phrase.length) continue;
    // The word after it, if the phrase ran to its length.
    if (!next) for (let j = phrase[phrase.length - 1] + 1; j < words.length && !next; j++) if (isWord(j)) next = lower[j];
    // "A narrow path climbs north": an -s word before a direction or "in" is a verb.
    if (phrase.length >= 2 && (AFTER_VERB.has(next) || DIRECTIONS.has(next) || AFTER_VERB_ADVERB.has(next)) && /[^s]s$/.test(lower[phrase[phrase.length - 1]])) phrase.pop();
    // "its lamp dark": the vague word isn't the noun.
    while (phrase.length > 1 && VAGUE.has(lower[phrase[phrase.length - 1]])) phrase.pop();
    heads.add(phrase[phrase.length - 1]);
  }
  const parts: Part[] = [];
  const nouns: string[] = [];
  for (let i = 0; i < words.length; i++) {
    const w = lower[i];
    if (!isWord(i)) {
      parts.push({ text: words[i], kind: "plain" });
      continue;
    }
    const what = knownAs(w);
    if (what) {
      parts.push({ text: words[i], kind: "known", what });
      continue;
    }
    const noun = singular(w);
    if (heads.has(i) && w.length > 2 && !VAGUE.has(w) && !DIRECTIONS.has(w) && !skip.has(w) && !skip.has(noun) && !w.endsWith("ly")) {
      parts.push({ text: words[i], kind: "new" });
      if (!nouns.includes(noun)) nouns.push(noun);
    } else parts.push({ text: words[i], kind: "plain" });
  }
  return { parts, nouns };
}

/** The words of things, as players can use them: name, synonyms, adjectives. */
function wordsOf(out: Map<string, string>, name: string, extra: string[]) {
  for (const w of [...name.toLowerCase().split(/[^a-z'-]+/), ...extra.map((x) => x.toLowerCase())]) {
    if (w.length > 1 && !DETERMINERS.has(w) && !STOP.has(w) && !out.has(w)) out.set(w, name);
  }
}

/** What's around a description's owner, by the words that name it. */
export function wordsHere(m: Model, owner: { room: string } | { thing: string } | { character: string }): { known: Map<string, string>; room: string | null } {
  const known = new Map<string, string>();
  const room = "room" in owner ? owner.room : "thing" in owner ? m.roomOf(owner.thing) : (m.character(owner.character)?.in ?? null);
  // The owner itself first, so its own words name it.
  if ("thing" in owner) {
    const t = m.thing(owner.thing);
    if (t) wordsOf(known, t.name, [...t.synonyms, ...t.adjectives]);
  } else if ("character" in owner) {
    const c = m.character(owner.character);
    if (c) wordsOf(known, c.name, c.synonyms);
  }
  if (room) {
    for (const t of m.things()) if (m.roomOf(t.id) === room) wordsOf(known, t.name, [...t.synonyms, ...t.adjectives]);
    for (const d of m.doors()) if (m.exits(room).some((e) => e.door === d.id)) wordsOf(known, d.name, []);
    for (const c of m.characters()) if (c.in === room) wordsOf(known, c.name, c.synonyms);
  }
  return { known, room };
}

/**
 * Under a description field: the words players can use, and nouns that
 * aren't anything yet. `input` is the field; it updates as it's typed in.
 */
export function describeWords(ctx: InspectorContext, owner: { room: string } | { thing: string } | { character: string }, input: HTMLInputElement | HTMLTextAreaElement): HTMLElement {
  const box = el("div", { class: "desc-words" });
  const draw = () => {
    const { known, room } = wordsHere(ctx.model, owner);
    const skip = new Set<string>();
    // The room's own name ("the kitchen") isn't a thing to add.
    if (room) for (const w of (ctx.model.room(room)?.name ?? "").toLowerCase().split(/\W+/)) skip.add(w);
    const { parts, nouns } = readDescription(input.value, known, skip);
    box.replaceChildren();
    if (!input.value.trim()) return;
    if (parts.some((p) => p.kind !== "plain")) {
      const preview = el("p", { class: "desc-preview" });
      for (const p of parts) {
        if (p.kind === "plain") preview.append(p.text);
        else {
          const span = el("span", { class: p.kind === "known" ? "kw" : "nw", textContent: p.text });
          span.title = p.kind === "known" ? `Players can use this: it's a word for ${p.what}` : "Not a thing yet: players may try it";
          preview.append(span);
        }
      }
      box.append(el("p", { class: "help", textContent: "Highlighted: words players can already use for something here. Underlined: things it mentions that aren't anything yet." }), preview);
    }
    if (nouns.length && room) {
      const row = el("div", { class: "desc-new" }, el("span", { class: "help", textContent: "Not things yet: " }));
      for (const n of nouns.slice(0, 8)) {
        const b = button(`+ ${n}`, () => ctx.change((x) => x.addThing("scenery", room, n), { label: `Added the ${n} as scenery` }), "small");
        b.title = `Add a ${n} to ${ctx.model.room(room)?.name ?? room} as scenery, so EXAMINE ${n.toUpperCase()} works`;
        row.append(b);
      }
      box.append(row);
    }
  };
  let timer = 0;
  input.addEventListener("input", () => {
    clearTimeout(timer);
    timer = window.setTimeout(draw, 250);
  });
  draw();
  return box;
}
