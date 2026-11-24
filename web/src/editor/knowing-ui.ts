// The editor's panels for what the player learns and owes: the game's
// facts, what teaches them, places heard of, favours characters ask, and
// copying.

import type { Model } from "./model.ts";
import { boolField, button, check, el, field, help, numField, select, type InspectorContext } from "./ui.ts";

const text = (ctx: InspectorContext, path: (string | number)[], label: string, opts: { multiline?: boolean; placeholder?: string; help?: string } = {}) =>
  field(label, String(ctx.model.get(path) ?? ""), (v) => ctx.change((x) => x.set(path, v.trim() ? v : undefined), { coalesce: path.join("."), redraw: false }), opts);

/** The game's facts, by key. */
export const factKeys = (m: Model) => m.pairs(["facts"]).map(([k]) => k);

/** A fact's words: `key: text` or `key: { text, place }`. */
function factText(m: Model, k: string): string {
  const v = m.get(["facts", k]);
  if (typeof v === "string") return v;
  const t = m.get(["facts", k, "text"]);
  return typeof t === "string" ? t : "";
}

/** A short name for a fact, for lists: its first words. */
function factLabel(m: Model, k: string): string {
  const t = factText(m, k);
  return t ? (t.length > 48 ? `${t.slice(0, 46)}…` : t) : k;
}

/** Sets a fact's text or place, keeping it as short as it can be written. */
function writeFact(ctx: InspectorContext, k: string, patch: { text?: string; place?: string }) {
  const m = ctx.model;
  const now = { text: factText(m, k), place: String(m.get(["facts", k, "place"]) ?? "") };
  const next = { ...now, ...patch };
  const p = ["facts", k];
  ctx.change((x) => (next.place ? x.set(p, { text: next.text, place: next.place }) : x.set(p, next.text || " ")), patch.text !== undefined ? { coalesce: `fact:${k}`, redraw: false } : undefined);
}

/** The game panel's list of facts: what each says, and the place it tells of. */
export function factsSection(ctx: InspectorContext): HTMLElement[] {
  const m = ctx.model;
  const keys = factKeys(m);
  const out: HTMLElement[] = [
    el("h3", { textContent: "What the player can learn" }),
    help("Facts the player learns in a conversation, or reading something: WHAT DO I KNOW lists the ones they have. A fact about a place puts it on their map with a “?”, and someone can tell them the way."),
  ];
  for (const k of keys) {
    out.push(
      el("h4", {}, k.replace(/_/g, " "), " ", button("✕", () => ctx.change((x) => {
        x.set(["facts", k], undefined);
        x.forgetFact(k);
      }), "small")),
      ...field("It says", factText(m, k), (v) => writeFact(ctx, k, { text: v }), { multiline: true, placeholder: "The smugglers meet at midnight." }),
      el("label", { textContent: "It tells of a place" }),
      select([["", "No place"], ...m.rooms().map((r): [string, string] => [r.id, r.name])], String(m.get(["facts", k, "place"]) ?? ""), (v) => writeFact(ctx, k, { place: v }), "It tells of a place"),
    );
  }
  const name = el("input", { type: "text", placeholder: "meeting", ariaLabel: "New fact" }) as HTMLInputElement;
  out.push(
    el(
      "div",
      { class: "add two" },
      name,
      button("Add a fact", () => {
        const k = name.value.trim().toLowerCase().replace(/[^a-z0-9_]+/g, "_").replace(/^_+|_+$/g, "");
        if (!k || keys.includes(k)) return;
        ctx.change((x) => (x.get(["facts"]) === undefined ? x.addSection("facts", { [k]: " " }, ["moods", "goals", "hints", "choices"]) : x.set(["facts", k], " ")));
      }),
    ),
    help("Then say where it's learned: on a conversation's part or reply, or on something to read."),
  );
  return out;
}

/** Ticks for which facts something teaches, at `path`. */
export function teachesField(ctx: InspectorContext, path: (string | number)[], label: string): HTMLElement[] {
  const m = ctx.model;
  const keys = factKeys(m);
  if (!keys.length) return [];
  const now = m.list(path);
  const box = el("div", { class: "radio column" });
  for (const k of keys) box.append(check(factLabel(m, k), now.includes(k), (v) => ctx.change((x) => x.setList(path, v ? [...now, k] : now.filter((f) => f !== k)))));
  return [el("label", { textContent: label }), box];
}

/** For an object: what it teaches, and copying. */
export function thingKnowing(ctx: InspectorContext, id: string): HTMLElement[] {
  const m = ctx.model;
  const p = ["objects", id];
  const others = m.things().filter((t) => t.id !== id);
  const copy = m.get([...p, "copy"]);
  return [
    ...teachesField(ctx, [...p, "teaches"], "Reading it teaches"),
    el("label", { textContent: "COPY makes" }),
    select([["", "Nothing (or its words, into the notes)"], ...others.map((t): [string, string] => [t.id, t.name])], typeof copy === "string" ? copy : "", (v) => ctx.change((x) => x.set([...p, "copy"], v || undefined)), "COPY makes"),
    help("A forgery: COPY THE PASS ONTO THE PAPER, with paper and a pen carried. Leave the copy unplaced. It passes for the real thing."),
    ...boolField(ctx, [...p, "blank"], "Something to copy onto (paper)", false),
    ...boolField(ctx, [...p, "writes"], "Something to write with (a pen)", false),
  ];
}

/** For a room: known from the start. */
export function roomKnowing(ctx: InspectorContext, id: string): HTMLElement[] {
  return boolField(ctx, ["rooms", id, "heard_of"], "The player knows of it from the start", false, "On their map with a “?” until they've been. GO TO it works once someone's told them the way.");
}

/** For a character: a favour they ask, and whether they give directions. */
export function characterKnowing(ctx: InspectorContext, id: string): HTMLElement[] {
  const m = ctx.model;
  const p = ["characters", id];
  const a = [...p, "asks"];
  const asks = m.get(a) !== undefined;
  const wants = m.get([...a, "for"]);
  const things = m.things();
  const talks = m.get([...p, "talk"]) !== undefined || ["merchant", "conversationalist"].includes(String(m.get([...p, "behavior"]) ?? ""));
  const out: HTMLElement[] = [
    el("h3", { textContent: "A favour they ask" }),
    el("label", { textContent: "They ask the player to bring" }),
    select(
      [["", "Nothing"], ...things.map((t): [string, string] => [t.id, t.name])],
      typeof wants === "string" ? wants : "",
      (v) => ctx.change((x) => (v ? x.set([...a, "for"], v) : x.set(a, undefined))),
      "They ask the player to bring",
    ),
  ];
  if (asks) {
    const gives = m.list([...a, "gives"]);
    const box = el("div", { class: "radio column" });
    for (const t of things.filter((t) => t.id !== wants)) box.append(check(t.name, gives.includes(t.id), (v) => ctx.change((x) => x.setList([...a, "gives"], v ? [...gives, t.id] : gives.filter((g) => g !== t.id)))));
    out.push(
      ...text(ctx, [...a, "say"], "How they ask (TALK TO them)", { multiline: true, placeholder: '"Have you seen my cat? Would you find her?"', help: "Said when the player talks to them, if they've no conversation." }),
      ...text(ctx, [...a, "promise"], "The player promises to", { placeholder: "find her cat", help: "For the journal: “You promised the baker you'd find her cat.”" }),
      ...text(ctx, [...a, "thanks"], "Given it, they say", { multiline: true, placeholder: '"Whiskers!" The baker hugs the cat.' }),
      el("label", { textContent: "And give the player" }),
      box,
      ...numField(ctx, [...a, "within"], "Promised, the player has (turns)", m.get([...a, "within"]), "Empty: as long as it takes."),
      ...text(ctx, [...a, "broken"], "If the promise is broken, they say", { multiline: true, placeholder: '"You said you\'d find her," says the baker coldly.' }),
      help("PROMISE (I'LL DO IT) promises it; so can a reply in a conversation. Kept, they're grateful; broken, they cool, and their friends hear of it."),
    );
  }
  out.push(...boolField(ctx, [...p, "knows_the_way"], "Gives directions when asked the way", talks, "ASK THEM THE WAY TO THE MILL. People with something to say, and shopkeepers, do unless you say not."));
  return out;
}
