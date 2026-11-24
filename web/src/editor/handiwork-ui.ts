// The editor's panels for working with your hands: ropes and what to tie
// them to, hollow hiding places, things that need both hands, mending and
// what's left when things break, food that goes bad, codes that jam, a
// doorkeeper's password, and the cold.

import type { Direction } from "./model.ts";
import { boolField, check, el, field, help, numField, select, type InspectorContext } from "./ui.ts";

const text = (ctx: InspectorContext, path: string[], label: string, opts: { multiline?: boolean; placeholder?: string; help?: string } = {}) =>
  field(label, String(ctx.model.get(path) ?? ""), (v) => ctx.change((x) => x.set(path, v.trim() ? v : undefined), { coalesce: path.join("."), redraw: false }), opts);

/** For an object: rope or anchor, hollow, two hands, warm, mending, going bad, what's left when it breaks, a code's tries. */
export function thingHandiwork(ctx: InspectorContext, id: string, traits: string[]): HTMLElement[] {
  const m = ctx.model;
  const p = ["objects", id];
  const has = (k: string) => m.get([...p, k]) !== undefined;
  const food = traits.includes("edible") || traits.includes("drinkable");
  const others = m.things().filter((t) => t.id !== id);
  const out: HTMLElement[] = [
    el("h3", { textContent: "Handiwork" }),
    ...boolField(ctx, [...p, "rope"], "A rope", false, "TIE IT TO something to tie to, to climb down a sheer drop, or TIE UP someone asleep, caught or cowering."),
    ...boolField(ctx, [...p, "anchor"], "Something to tie a rope to (a tree, a post, a hook)", false),
    ...boolField(ctx, [...p, "hollow"], "Sounds hollow when knocked on", false, "Start it hidden: KNOCK ON THE WALLS (or on what it's in) finds it. A secret panel, a loose stone."),
    ...boolField(ctx, [...p, "two_handed"], "Takes both hands to carry", false, "Only in a game where hands count (the player's details)."),
    ...boolField(ctx, [...p, "warm"], traits.includes("wearable") ? "Keeps the cold out, worn (a coat)" : food ? "Warms the player up, eaten or drunk (soup)" : "Warm (worn: keeps out the cold; eaten: warms you up)", false),
    ...boolField(ctx, [...p, "mends"], "Mends things (glue, needle and thread)", false, "MEND THE NET WITH IT: worn things as good as new, broken ones whole. Give it uses to use it up."),
  ];
  if (food) {
    out.push(...numField(ctx, [...p, "spoils"], "Goes bad after (turns)", m.get([...p, "spoils"]), "Counted from the start, while it's anywhere in the game. Then it won't be eaten.", "never"));
  }
  if (has("uses")) {
    const left = m.get([...p, "breaks_into"]);
    out.push(
      el("label", { textContent: "When it breaks, it leaves" }),
      select([["", "Nothing"], ...others.map((t): [string, string] => [t.id, t.name])], typeof left === "string" ? left : "", (v) => ctx.change((x) => x.set([...p, "breaks_into"], v || undefined)), "When it breaks, it leaves"),
      help("A broken key: leave it unplaced. Something that mends can make it whole again."),
    );
  }
  if (has("code")) {
    out.push(
      ...numField(ctx, [...p, "tries"], "Jams after this many wrong codes", m.get([...p, "tries"]), "Jammed, it raises the alarm nearby. Empty: as many as they like.", "no limit"),
      ...boolField(ctx, [...p, "feedback"], "Says how close a wrong code was", false, "“2 in the right place, and 1 more right but in the wrong place.”"),
    );
  }
  return out;
}

/** For a room: bitterly cold. */
export function roomHandiwork(ctx: InspectorContext, id: string): HTMLElement[] {
  return boolField(ctx, ["rooms", id, "cold"], "Bitterly cold", false, "The player lasts a few turns, unless they wear something warm or there's a fire. Warm food helps.");
}

/** For an exit: a sheer drop that needs a rope. */
export function exitRope(ctx: InspectorContext, room: string, dir: Direction, on: boolean): HTMLElement[] {
  return [
    el("h3", { textContent: "A sheer drop" }),
    check("Only with a rope tied at the top (or the bottom)", on, (v) => ctx.change((x) => x.setRope(room, dir, v))),
    help("Give an object here “Something to tie a rope to”, and a rope somewhere. CLIMB THE ROPE goes this way."),
  ];
}

/** For the player: hands, and warmth in a game with cold. */
export function playerHandiwork(ctx: InspectorContext): HTMLElement[] {
  const m = ctx.model;
  const counted = m.get(["player", "hands"]) !== undefined;
  const cold = m.rooms().some((r) => m.get(["rooms", r.id, "cold"]) === true) || Object.values((m.get(["weather", "kinds"]) ?? {}) as Record<string, { cold?: unknown }>).some((k) => k?.cold === true);
  return [
    el("h3", { textContent: "Hands" }),
    check("Hands count: only so much can be held at once", counted, (v) => ctx.change((x) => x.set(["player", "hands"], v ? 2 : undefined))),
    ...(counted
      ? [
          ...numField(ctx, ["player", "hands"], "Hands", m.get(["player", "hands"]), "Big things (size 2 or more), weapons and burning lights take a hand; small things go in pockets. Climbing takes a hand free.", "2"),
        ]
      : []),
    ...(cold ? [el("h3", { textContent: "The cold" }), ...numField(ctx, ["player", "warmth"], "The player lasts in the cold for (turns)", m.get(["player", "warmth"]), "Then it hurts each turn, or, if they can't be hurt, they freeze.", "8")] : []),
  ];
}

/** For a character: a password they want before they let the player by. */
export function characterHandiwork(ctx: InspectorContext, id: string): HTMLElement[] {
  const m = ctx.model;
  const p = ["characters", id, "password"];
  const on = m.get(p) !== undefined;
  const out: HTMLElement[] = [
    el("h3", { textContent: "A password" }),
    check("Won't let the player by without a password", on, (v) => ctx.change((x) => x.set(p, v ? { word: "swordfish", bars: "north" } : undefined))),
  ];
  if (!on) return out;
  const home = m.get(["characters", id, "in"]);
  const dirs = typeof home === "string" ? m.exits(home).map((e) => e.dir) : [];
  const bars = m.list([...p, "bars"]).length ? m.list([...p, "bars"]) : typeof m.get([...p, "bars"]) === "string" ? [String(m.get([...p, "bars"]))] : [];
  const box = el("div", { class: "radio" });
  for (const d of dirs) {
    box.append(
      check(d, bars.includes(d), (v) =>
        ctx.change((x) => {
          const next = v ? [...bars, d] : bars.filter((b) => b !== d);
          x.set([...p, "bars"], next.length === 1 ? next[0] : next.length ? next : undefined);
        }),
      ),
    );
  }
  out.push(
    ...text(ctx, [...p, "word"], "The password", { placeholder: "swordfish", help: "SAY SWORDFISH. Hide it in a clue, or a conversation." }),
    el("label", { textContent: "The ways they bar" }),
    dirs.length ? box : help("Put them in a room with ways out first."),
    ...text(ctx, [...p, "ask"], "Stopping the player, they say", { placeholder: 'The guard steps in your way. "Password?"' }),
    ...text(ctx, [...p, "right"], "Given it, they say", { placeholder: "The guard nods, and stands aside." }),
    ...text(ctx, [...p, "wrong"], "A wrong one, they say", { placeholder: '"That\'s not it," says the guard.' }),
    help("Asleep or tied up, they let anyone by."),
  );
  return out;
}

/** For a kind of weather: bitterly cold out of doors. */
export function weatherCold(ctx: InspectorContext, kind: string): HTMLElement[] {
  return boolField(ctx, ["weather", "kinds", kind, "cold"], "Bitterly cold out of doors", false);
}
