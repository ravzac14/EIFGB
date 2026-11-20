// The editor's panels for how things behave: flames and fuel, liquids,
// wear, codes, writing, sounds and smells; outdoor rooms; the weather.

import { boolField, button, el, field, help, numField, select, type InspectorContext } from "./ui.ts";

const text = (ctx: InspectorContext, path: string[], label: string, opts: { multiline?: boolean; placeholder?: string; help?: string } = {}) =>
  field(label, String(ctx.model.get(path) ?? ""), (v) => ctx.change((x) => x.set(path, v.trim() ? v : undefined), { coalesce: path.join("."), redraw: false }), opts);

/** For an object: light and fuel (if it's a light), liquids, wear, a code, writing, sound and smell. */
export function thingBehaviour(ctx: InspectorContext, id: string, traits: string[]): HTMLElement[] {
  const m = ctx.model;
  const p = ["objects", id];
  const light = traits.includes("light_source");
  const has = (k: string) => m.get([...p, k]) !== undefined;
  const open = ["needs_flame", "fuel", "holds_liquid", "liquid", "uses", "code", "text", "sound", "smell"].some(has);
  return [
    el(
      "details",
      { class: "more", open },
      el("summary", { textContent: "More: light, liquid, wear, a code, writing, sounds" }),
      ...(light
        ? [
            ...boolField(ctx, [...p, "needs_flame"], "Needs a flame to light (a candle, a torch)", false, "LIGHT IT WITH THE LIGHTER: something burning, held. A lighter or match doesn't need one."),
            ...numField(ctx, [...p, "fuel"], "Burns for (turns)", m.get([...p, "fuel"]), "Empty: for ever. It flickers two turns before it goes out.", "for ever"),
            ...(has("fuel") ? boolField(ctx, [...p, "burns_away"], "Gone when it burns out (a match)", false) : []),
            ...(has("fuel") ? text(ctx, [...p, "spent"], "When it burns out, it says", { placeholder: "The match burns down to your fingers." }) : []),
          ]
        : []),
      ...boolField(ctx, [...p, "holds_liquid"], "Holds a liquid (a bottle, a bucket)", false, "FILL IT FROM THE STREAM, POUR IT ON THE FIRE, DRINK IT."),
      ...(m.bool([...p, "holds_liquid"], false) ? text(ctx, [...p, "full_of"], "Starts full of", { placeholder: "water" }) : []),
      ...text(ctx, [...p, "liquid"], "A liquid comes from it", { placeholder: "water", help: "For a stream, a well, a barrel: players fill things from it. Water puts out fires." }),
      ...numField(ctx, [...p, "uses"], "Breaks after this many uses", m.get([...p, "uses"]), "Each time it's used as a tool (a key, a knife, a rope with a handler). Empty: never.", "never"),
      ...(has("uses") ? text(ctx, [...p, "breaks"], "When it breaks, it says", { placeholder: "The rope frays and snaps." }) : []),
      ...(traits.includes("lockable") ? text(ctx, [...p, "code"], "A code unlocks it", { placeholder: "4512", help: "TYPE 4512, or OPEN THE SAFE WITH 4512. Hide the code in a clue." }) : []),
      ...text(ctx, [...p, "text"], "What's written on it (READ)", { multiline: true, placeholder: "\"Meet me at the old mill.\"" }),
      ...text(ctx, [...p, "sound"], "What LISTEN hears of it", { placeholder: "It ticks steadily." }),
      ...text(ctx, [...p, "smell"], "What SMELL smells of it", { placeholder: "It smells of pine." }),
    ),
  ];
}

/** For a room: out of doors, its sound and smell (heard and smelt from next door too). */
export function roomBehaviour(ctx: InspectorContext, id: string): HTMLElement[] {
  const p = ["rooms", id];
  return [
    ...boolField(ctx, [...p, "outdoors"], "Out of doors (the weather shows here)", false),
    ...text(ctx, [...p, "sound"], "What LISTEN hears here", { placeholder: "Water drips somewhere.", help: "Heard from the rooms next door, too." }),
    ...text(ctx, [...p, "smell"], "What SMELL smells here", { placeholder: "A musty smell of bats." }),
  ];
}

/** The game's weather: its kinds, the first, and how often it changes. */
export function weatherSection(ctx: InspectorContext): HTMLElement[] {
  const m = ctx.model;
  const w = m.get(["weather"]) as { start?: string; changes?: unknown; kinds?: Record<string, unknown> } | undefined;
  if (!w) {
    return [
      help("Weather that outdoor rooms describe, and that can change by itself or from a script: set_weather(\"rain\")."),
      el(
        "div",
        { class: "row" },
        button("Add weather", () =>
          ctx.change((x) =>
            x.set(["weather"], {
              start: "clear",
              changes: "20-40",
              kinds: { clear: { describe: "The sky is clear.", begins: "The clouds clear." }, rain: { describe: "Rain falls steadily.", begins: "It starts to rain." } },
            }),
          ),
        ),
      ),
    ];
  }
  const kinds = Object.keys(w.kinds ?? {});
  const name = el("input", { type: "text", placeholder: "fog", ariaLabel: "New kind of weather" }) as HTMLInputElement;
  return [
    el("label", { textContent: "It starts" }),
    select(kinds.map((k): [string, string] => [k, k]), w.start ?? kinds[0] ?? "", (v) => ctx.change((x) => x.set(["weather", "start"], v)), "Starting weather"),
    ...field("It changes every (turns)", String(w.changes ?? ""), (v) => ctx.change((x) => x.set(["weather", "changes"], v.trim() || undefined), { coalesce: "weather.changes", redraw: false }), {
      placeholder: "20-40",
      help: "A number, or a range like 20-40 for a random wait. Empty: only scripts change it.",
    }),
    ...kinds.flatMap((k) => [
      el("h4", {}, k, " ", button("✕", () => ctx.change((x) => x.set(["weather", "kinds", k], undefined)), "small")),
      ...text(ctx, ["weather", "kinds", k, "describe"], "Outdoors, rooms add", { placeholder: "Rain falls steadily." }),
      ...text(ctx, ["weather", "kinds", k, "begins"], "When it starts, it says", { placeholder: "It starts to rain." }),
    ]),
    el(
      "div",
      { class: "add two" },
      name,
      button("Add a kind", () => {
        const k = name.value.trim().toLowerCase().replace(/[^a-z0-9_]+/g, "_");
        if (k) ctx.change((x) => x.set(["weather", "kinds", k], {}));
      }),
    ),
    el("div", { class: "row" }, button("Remove the weather", () => ctx.change((x) => x.set(["weather"], undefined)), "danger")),
  ];
}
