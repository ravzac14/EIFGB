// The editor's panels for the cunning tricks: moods (how rooms, things and
// people read while the player is drunk or dreaming), reaching, mirrors,
// taking things apart, voices, friends, memories, and the characters the
// player can play as.

import type { Model } from "./model.ts";
import { boolField, button, check, el, field, help, numField, select, type InspectorContext } from "./ui.ts";

const text = (ctx: InspectorContext, path: string[], label: string, opts: { multiline?: boolean; placeholder?: string; help?: string } = {}) =>
  field(label, String(ctx.model.get(path) ?? ""), (v) => ctx.change((x) => x.set(path, v.trim() ? v : undefined), { coalesce: path.join("."), redraw: false }), opts);

/** Moods named while this editor's open, before any text is written for them. */
const named = new Set<string>();

/** The keys of a map in the file, or none. */
const keysOf = (m: Model, path: string[]) => m.pairs(path).map(([k]) => k);

/** Every mood the game knows: written for somewhere, brought on by something, or just named. */
export function moodNames(m: Model): string[] {
  const out = new Set<string>(keysOf(m, ["moods"]));
  for (const id of m.roomIds()) for (const k of keysOf(m, ["rooms", id, "moods"])) out.add(k);
  for (const s of ["objects", "characters"]) {
    for (const id of m.ids(s)) {
      for (const k of keysOf(m, [s, id, "moods"])) out.add(k);
      for (const k of keysOf(m, [s, id, "mood"])) out.add(k);
    }
  }
  for (const k of named) out.add(k);
  return [...out];
}

/** A mood's name as an ID: "Very drunk" is very_drunk. */
const moodId = (s: string) => s.trim().toLowerCase().replace(/[^a-z0-9_]+/g, "_").replace(/^_+|_+$/g, "");

/** A row to name a new mood. */
function addMoodRow(ctx: InspectorContext): HTMLElement {
  const name = el("input", { type: "text", placeholder: "drunk", ariaLabel: "New mood" }) as HTMLInputElement;
  return el(
    "div",
    { class: "add two" },
    name,
    button("Add a mood", () => {
      const k = moodId(name.value);
      if (!k) return;
      named.add(k);
      ctx.change(() => {});
    }),
  );
}

/**
 * The text for each mood at `owner` (a room, thing or character), or with
 * `owner` empty the game's own lines, added to LOOK everywhere.
 */
export function moodsSection(ctx: InspectorContext, owner: string[]): HTMLElement[] {
  const m = ctx.model;
  const game = owner.length === 0;
  const moods = moodNames(m);
  const out: HTMLElement[] = [el("h3", { textContent: game ? "Moods" : "In a mood" })];
  out.push(
    help(
      game
        ? "While the player is drunk, dreaming or frightened, rooms, things and people can read differently. Name a mood here; something eaten or drunk brings it on (in that thing's panel), or a script: player.state.drunk = true. Each line here is added to LOOK, in rooms without their own words for it."
        : "While the player is in this mood, this is said instead of the description, like a room in the dark.",
    ),
  );
  for (const k of moods) {
    out.push(
      ...text(ctx, [...owner, "moods", k], game ? `While ${k}, LOOK adds` : `While the player is ${k}`, {
        multiline: !game,
        placeholder: game ? "The floor sways gently." : "",
      }),
    );
  }
  out.push(addMoodRow(ctx));
  return out;
}

/** The parts something comes apart into, and what's said. */
function splits(m: Model, id: string): { parts: string[]; say: string } {
  const p = ["objects", id, "splits_into"];
  const v = m.get(p);
  if (v && typeof v === "object" && !Array.isArray(v) && "items" in v && m.get([...p, "parts"]) !== undefined) {
    const say = m.get([...p, "say"]);
    return { parts: m.list([...p, "parts"]), say: typeof say === "string" ? say : "" };
  }
  return { parts: m.list(p), say: "" };
}

function writeSplits(ctx: InspectorContext, id: string, parts: string[], say: string) {
  const p = ["objects", id, "splits_into"];
  ctx.change((x) => {
    if (!parts.length) return x.set(p, undefined);
    if (!say.trim()) return x.setList(p, parts);
    x.set(p, undefined);
    x.setList([...p, "parts"], parts);
    x.set([...p, "say"], say);
  });
}

/** For an object: reaching, mirrors, coming apart, and a mood it brings on. */
export function thingTricks(ctx: InspectorContext, id: string, traits: string[]): HTMLElement[] {
  const m = ctx.model;
  const p = ["objects", id];
  const out: HTMLElement[] = [
    el("h3", { textContent: "Tricks" }),
    ...boolField(ctx, [...p, "reaches"], "Reaches far and high things (a pole, a hook)", false, "Held, it reaches what's far off or up high: HOOK THE NEST WITH THE POLE."),
    ...boolField(ctx, [...p, "reflects"], "A mirror", false, "LOOK EAST WITH THE MIRROR sees the next room. ANGLE THE MIRROR NORTH, in a lit room, throws light into a dark one next door, while it's left there."),
  ];

  // Taking it apart.
  const { parts, say } = splits(m, id);
  const others = m.things().filter((t) => t.id !== id && !parts.includes(t.id));
  const name = (pid: string) => m.thing(pid)?.name ?? pid;
  out.push(el("label", { textContent: "Comes apart into" }));
  if (parts.length) {
    out.push(
      el(
        "ul",
        { class: "parts" },
        ...parts.map((pid) => el("li", {}, name(pid), " ", button("✕", () => writeSplits(ctx, id, parts.filter((x) => x !== pid), say), "small"))),
      ),
    );
  }
  out.push(
    select(
      [["", parts.length ? "Add another part…" : "Nothing: it stays whole"], ...others.map((t): [string, string] => [t.id, t.name])],
      "",
      (v) => v && writeSplits(ctx, id, [...parts, v], say),
      "Add a part",
    ),
    help("TAKE IT APART leaves these instead. Leave the parts unplaced (Not placed anywhere yet, in the game's panel) so they only turn up then."),
  );
  if (parts.length) {
    out.push(
      ...field("When it comes apart, it says", say, (v) => writeSplits(ctx, id, parts, v), {
        placeholder: "You take it apart.",
      }),
    );
  }

  // A mood it brings on, eaten or drunk.
  if (traits.includes("edible") || traits.includes("drinkable")) {
    const brought = m.pairs([...p, "mood"])[0];
    const moods = moodNames(m);
    const current = brought?.[0] ?? "";
    const turns = brought ? Number.parseInt(brought[1], 10) : NaN;
    const setMood = (k: string, n: number) => ctx.change((x) => x.set([...p, "mood"], k ? { [k]: Number.isFinite(n) ? n : 0 } : undefined));
    out.push(
      el("label", { textContent: traits.includes("drinkable") ? "Drinking it brings on a mood" : "Eating it brings on a mood" }),
      select([["", "None"], ...moods.map((k): [string, string] => [k, k])], current, (v) => setMood(v, turns), "Mood"),
    );
    if (!moods.length) out.push(help("Name a mood first, under Moods in the game's panel."));
    if (current) {
      const i = el("input", { type: "text", inputMode: "numeric", value: Number.isFinite(turns) && turns > 0 ? String(turns) : "", placeholder: "for good" }) as HTMLInputElement;
      i.addEventListener("input", () => {
        const n = Number.parseInt(i.value, 10);
        ctx.change((x) => x.set([...p, "mood", current], Number.isFinite(n) && n > 0 ? n : 0), { coalesce: `mood:${id}`, redraw: false });
      });
      out.push(el("label", { textContent: "For this many turns" }), i, help("Empty: until a script ends it."));
    }
  }
  return out;
}

/** A tick list of the other characters, for a list of IDs at `path`. */
function peopleTicks(ctx: InspectorContext, id: string, path: string[]): HTMLElement {
  const m = ctx.model;
  const now = m.list(path);
  const box = el("div", { class: "radio column" });
  for (const c of m.characters()) {
    if (c.id === id) continue;
    box.append(check(c.name, now.includes(c.id), (v) => ctx.change((x) => x.setList(path, v ? [...now, c.id] : now.filter((f) => f !== c.id)))));
  }
  return box;
}

/** What a character can remember the player doing to them. */
const DEEDS: [string, string, string][] = [
  ["gift", "You gave them something", "Oh, it's you again. Thank you for the bread."],
  ["attacked", "You attacked them", "You! Keep your distance."],
  ["robbed", "They caught you stealing", "I'm watching my pockets this time."],
  ["spared", "They ran from a fight with you", "You let me go, before. I won't forget it."],
  ["fooled", "You fooled them with someone's voice", "That was you, calling like the captain. Clever."],
];

/** For a character: whether the player can play as them, voices, friends, memories. */
export function characterTricks(ctx: InspectorContext, id: string): HTMLElement[] {
  const m = ctx.model;
  const p = ["characters", id];
  const others = m.characters().filter((c) => c.id !== id);
  const playable = m.bool([...p, "playable"], false);
  const out: HTMLElement[] = [
    el("h3", { textContent: "Playing as them" }),
    check("The player can play as them", playable, (v) =>
      ctx.change((x) => {
        x.setBool([...p, "playable"], v, false);
        const starts = x.get(["player", "plays"]);
        if (v && !starts) {
          if (x.get(["player"]) === undefined) x.addSection("player", { plays: id }, ["game", "settings", "vars"]);
          else x.set(["player", "plays"], id);
        }
        if (!v && starts === id) {
          const next = x.characterIds().find((c) => c !== id && x.bool(["characters", c, "playable"], false));
          x.set(["player", "plays"], next);
        }
      }),
    ),
    help("SWITCH TO them, between the characters the player can play. The player becomes them, where they are, with what they carry."),
  ];
  if (playable) out.push(...startsAs(ctx));
  if (others.length) {
    out.push(
      el("h3", { textContent: "Voices and friends" }),
      el("label", { textContent: "Comes running to the voice of" }),
      peopleTicks(ctx, id, [...p, "answers_to"]),
      help("Once the player has heard that one talk, CALL OUT AS THE CAPTAIN brings this one from up to two rooms away, if they can't see who called."),
      el("label", { textContent: "Friends with" }),
      peopleTicks(ctx, id, [...p, "friends"]),
      help("What they think of the player spreads to their friends a few turns later."),
    );
  }
  out.push(el("h3", { textContent: "What they remember" }), help("Said once, the next time they meet the player after it (only the latest)."));
  for (const [deed, label, eg] of DEEDS) out.push(...text(ctx, [...p, "remembers", deed], label, { placeholder: eg }));
  return out;
}

/** Who the player starts as, among the playable characters (for the game's panel, too). */
export function startsAs(ctx: InspectorContext): HTMLElement[] {
  const m = ctx.model;
  const playable = m.characters().filter((c) => m.bool(["characters", c.id, "playable"], false));
  if (!playable.length) return [];
  const plays = m.get(["player", "plays"]);
  return [
    el("label", { textContent: "The player starts as" }),
    select(
      playable.map((c): [string, string] => [c.id, c.name]),
      typeof plays === "string" ? plays : "",
      (v) => ctx.change((x) => x.set(["player", "plays"], v)),
      "The player starts as",
    ),
  ];
}

// ------------------------------------------------- the earlier tricks

/** A list of IDs picked one at a time from `options`, each with a ✕. */
function picker(now: string[], options: [string, string][], label: string, empty: string, write: (ids: string[]) => void): HTMLElement[] {
  const name = (id: string) => options.find(([k]) => k === id)?.[1] ?? id;
  const out: HTMLElement[] = [el("label", { textContent: label })];
  if (now.length) out.push(el("ul", { class: "parts" }, ...now.map((id) => el("li", {}, name(id), " ", button("✕", () => write(now.filter((x) => x !== id)), "small")))));
  out.push(select([["", now.length ? "Add another…" : empty], ...options.filter(([k]) => !now.includes(k))], "", (v) => v && write([...now, v]), label));
  return out;
}

/** A list of IDs at `path`, picked from the game's things. */
const thingPicker = (ctx: InspectorContext, path: string[], label: string, empty: string, but?: string) =>
  picker(ctx.model.list(path), ctx.model.things().filter((t) => t.id !== but).map((t): [string, string] => [t.id, t.name]), label, empty, (ids) => ctx.change((x) => x.setList(path, ids)));

interface Combine {
  with: string;
  makes: string;
  say?: string;
  keep?: string | string[];
  by?: string | string[];
}

/** What a thing combines into: `combines:`, one or a list. */
function combinesOf(m: Model, id: string): Combine[] {
  const v = m.get(["objects", id, "combines"]) as { toJSON?: () => unknown } | undefined;
  const j = v?.toJSON?.();
  return (Array.isArray(j) ? j : j ? [j] : []) as Combine[];
}

function writeCombines(ctx: InspectorContext, id: string, list: Combine[], typing?: string) {
  const clean = list.map((c) => Object.fromEntries(Object.entries(c).filter(([, v]) => v !== undefined && v !== "" && !(Array.isArray(v) && !v.length))));
  ctx.change((x) => x.set(["objects", id, "combines"], clean.length === 0 ? undefined : clean.length === 1 ? clean[0] : clean), typing ? { coalesce: typing, redraw: false } : undefined);
}

/** For an object: combining, marks, decoys, putting out fires, traps, disguises, bait. */
export function thingEarlierTricks(ctx: InspectorContext, id: string, traits: string[]): HTMLElement[] {
  const m = ctx.model;
  const p = ["objects", id];
  const things = m.things().filter((t) => t.id !== id);
  const name = (tid: string) => m.thing(tid)?.name ?? tid;
  const out: HTMLElement[] = [];

  // Combining.
  const list = combinesOf(m, id);
  out.push(el("label", { textContent: "Combines with something else" }));
  list.forEach((c, i) => {
    const set = (patch: Partial<Combine>) => writeCombines(ctx, id, list.map((x, j) => (j === i ? { ...x, ...patch } : x)));
    const keep = typeof c.keep === "string" ? [c.keep] : (c.keep ?? []);
    const keepBox = (who: string) =>
      check(`Keep the ${name(who)}`, keep.includes(who), (v) => set({ keep: v ? [...keep, who] : keep.filter((k) => k !== who) }));
    out.push(
      el(
        "div",
        { class: "indent combine" },
        el("label", { textContent: "With" }),
        select([["", "choose…"], ...things.map((t): [string, string] => [t.id, t.name])], c.with ?? "", (v) => set({ with: v }), "Combines with"),
        el("label", { textContent: "Makes" }),
        select([["", "choose…"], ...things.map((t): [string, string] => [t.id, t.name])], c.makes ?? "", (v) => set({ makes: v }), "Makes"),
        ...field("It says", c.say ?? "", (v) => {
          list[i] = { ...c, say: v || undefined };
          writeCombines(ctx, id, list, `combine-say:${id}:${i}`);
        }, { placeholder: "You tie the hook to the rope." }),
        keepBox(id),
        ...(c.with ? [keepBox(c.with)] : []),
        el("div", { class: "row" }, button("Remove", () => writeCombines(ctx, id, list.filter((_, j) => j !== i)), "small")),
      ),
    );
  });
  out.push(
    el("div", { class: "row" }, button(list.length ? "Add another" : "Add one", () => writeCombines(ctx, id, [...list, { with: "", makes: "" }]), "small")),
    help("TIE THE HOOK TO THE ROPE, COMBINE THEM, PUT ONE IN THE OTHER: both go, and what they make appears. Leave what it makes unplaced, so it only turns up then."),
  );

  out.push(
    ...boolField(ctx, [...p, "marks"], "Leaves a mark (chalk, a knife)", false, "MARK X, CHALK AN ARROW: a sign in the room, for finding the way back through a maze."),
    el("label", { textContent: "A fake of" }),
    select([["", "Nothing"], ...things.map((t): [string, string] => [t.id, t.name])], typeof m.get([...p, "decoy_for"]) === "string" ? String(m.get([...p, "decoy_for"])) : "", (v) => ctx.change((x) => x.set([...p, "decoy_for"], v || undefined)), "A fake of"),
    help("Good enough to SWAP for it under a watcher's nose: REPLACE THE IDOL WITH THE BAG OF SAND."),
    ...boolField(ctx, [...p, "douses"], "Puts out fires (water, sand)", false, "POUR IT ON THE FIRE. Used up doing it."),
  );

  // A trap.
  const holds = m.get([...p, "trap", "holds"]);
  out.push(
    check("A trap", typeof holds === "number", (v) => ctx.change((x) => x.set([...p, "trap"], v ? { holds: 3 } : undefined))),
    help("SET THE TRAP: what walks into it is held for a while."),
  );
  if (typeof holds === "number") out.push(el("div", { class: "indent" }, ...numField(ctx, [...p, "trap", "holds"], "Holds what it catches for (turns)", holds, "", "3")));

  // A disguise, when worn; bait, for a side.
  const sides = m.factionIds();
  if (traits.includes("wearable") && sides.length) {
    const d = m.get([...p, "disguise"]);
    out.push(
      el("label", { textContent: "Worn, it's a disguise as one of" }),
      select([["", "Nobody"], ...sides.map((f): [string, string] => [f, `the ${f}`])], typeof d === "string" ? d : "", (v) => ctx.change((x) => x.set([...p, "disguise"], v || undefined)), "Disguise"),
      help("They take the player for one of their own, unless the player has already crossed them."),
    );
  }
  if (sides.length) {
    const eaten = m.list([...p, "eaten_by"]);
    const box = el("div", { class: "radio column" });
    for (const f of sides) box.append(check(`The ${f}`, eaten.includes(f), (v) => ctx.change((x) => x.setList([...p, "eaten_by"], v ? [...eaten, f] : eaten.filter((e) => e !== f)))));
    out.push(el("label", { textContent: "Eaten by, left lying where they are" }), box, help("Bait: drop the meat and the wolves stop to eat it."));
  }
  return out;
}

/** For a character: what they watch, and what wins them over. */
export function characterWants(ctx: InspectorContext, id: string): HTMLElement[] {
  const p = ["characters", id];
  return [
    el("h3", { textContent: "What they watch, and what wins them over" }),
    ...thingPicker(ctx, [...p, "watches"], "Won't let the player take", "Nothing"),
    help("While they're here and looking. Distract them, or SWAP it for a fake."),
    ...thingPicker(ctx, [...p, "likes"], "Likes being given", "Nothing in particular"),
    help("A gift they like warms them to the player."),
    ...thingPicker(ctx, [...p, "tame_with"], "Tamed by being given", "Nothing"),
    help("Tamed, a creature stops fighting the player, and may follow and fetch."),
    ...numField(ctx, [...p, "bribe"], "Can be bribed with (money)", ctx.model.get([...p, "bribe"]), "BRIBE THE GUARD WITH 5. Empty: they can't be bought."),
  ];
}
