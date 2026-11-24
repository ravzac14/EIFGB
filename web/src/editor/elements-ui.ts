// The editor's panels for the physical world: water rising in a room, an
// echo and a whispering gallery; pressure plates, things that sink or
// float, and scents for a dog to follow.

import { boolField, button, check, el, field, help, numField, select, type InspectorContext } from "./ui.ts";

/** Pairs of words and answers at `path`, one a line: `help: "Down here!"`. */
function answersField(ctx: InspectorContext, path: string[]): HTMLElement[] {
  const pairs = ctx.model.pairs(path);
  return field("It answers", pairs.map(([k, v]) => `${k}: ${v}`).join("\n"), (v) =>
    ctx.change(
      (x) =>
        x.setPairs(
          path,
          v.split("\n").map((l): [string, string] => {
            const i = l.indexOf(":");
            return i < 0 ? [l, ""] : [l.slice(0, i), l.slice(i + 1)];
          }),
        ),
      { coalesce: path.join("."), redraw: false },
    ), {
    multiline: true,
    placeholder: 'help: "…help…" And then, faintly: "Down here!"',
    help: "One a line: a word shouted or said here, and what comes back. Anything else just echoes.",
  });
}

/** For a room: water, an echo, and rooms whose talk carries here. */
export function roomElements(ctx: InspectorContext, id: string): HTMLElement[] {
  const m = ctx.model;
  const p = ["rooms", id];
  const floods = m.get([...p, "floods"]) !== undefined;
  const echo = m.get([...p, "echo"]);
  const out: HTMLElement[] = [
    el("h3", { textContent: "Water and sound" }),
    check("Water rises here", floods, (v) => ctx.change((x) => x.set([...p, "floods"], v ? { every: 3, depth: 3 } : undefined))),
  ];
  if (floods) {
    const openable = m.things().filter((t) => t.traits.includes("openable"));
    out.push(
      el(
        "div",
        { class: "indent" },
        ...numField(ctx, [...p, "floods", "every"], "It rises a level every (turns)", m.get([...p, "floods", "every"]), "", "3"),
        el("label", { textContent: "Up to" }),
        select(
          [["1", "Ankle-deep"], ["2", "Waist-deep"], ["3", "Over the player's head"]],
          String(m.get([...p, "floods", "depth"]) ?? "3"),
          (v) => ctx.change((x) => x.set([...p, "floods", "depth"], Number(v))),
          "Up to",
        ),
        ...boolField(ctx, [...p, "floods", "starts"], "Rising from the start", true, "Otherwise a script starts it: flood(world.cellar)."),
        el("label", { textContent: "Drained by opening" }),
        select(
          [["", "Nothing (or a script: flood(world.cellar, 0))"], ...openable.map((t): [string, string] => [t.id, t.name])],
          String(m.get([...p, "floods", "drains"]) ?? ""),
          (v) => ctx.change((x) => x.set([...p, "floods", "drains"], v || undefined)),
          "Drained by opening",
        ),
        help("Over the player's head, they hold their breath, and swim (SWIM NORTH). Something that floats, held, keeps them up."),
      ),
    );
  }
  out.push(check("An echo", echo !== undefined && echo !== false, (v) => ctx.change((x) => x.set([...p, "echo"], v || undefined))));
  if (echo !== undefined && echo !== false) {
    // `echo: true` becomes a map once there are answers.
    const answers = typeof echo === "object" ? answersField(ctx, [...p, "echo"]) : [];
    out.push(
      el(
        "div",
        { class: "indent" },
        help("SHOUT HELLO or SAY HELLO here: “…hello… hello…” comes back."),
        ...(typeof echo === "object"
          ? answers
          : [el("div", { class: "actions" }, button("Add answers", () => ctx.change((x) => x.set([...p, "echo"], { help: "…help… help…" })), "small"))]),
      ),
    );
  }
  const others = m.rooms().filter((r) => r.id !== id);
  if (others.length) {
    const hears = m.list([...p, "hears"]);
    const box = el("div", { class: "radio column" });
    for (const r of others) box.append(check(r.name, hears.includes(r.id), (v) => ctx.change((x) => x.setList([...p, "hears"], v ? [...hears, r.id] : hears.filter((h) => h !== r.id)))));
    out.push(el("label", { textContent: "What's said in these rooms is heard here" }), box, help("A whispering gallery: characters' talk there carries here, word for word."));
  }
  return out;
}

/** For an object: a pressure plate, sinking, floating, a scent. */
export function thingElements(ctx: InspectorContext, id: string, traits: string[]): HTMLElement[] {
  const m = ctx.model;
  const p = ["objects", id];
  const plate = m.get([...p, "pressure"]) !== undefined;
  const holds = traits.includes("supporter") || traits.includes("container") || traits.includes("enterable");
  const out: HTMLElement[] = [];
  if (holds) {
    out.push(check("A pressure plate", plate, (v) => ctx.change((x) => x.set([...p, "pressure"], v ? { at_least: 3 } : undefined))));
    if (plate) {
      const opens = m.list([...p, "pressure", "opens"]);
      const box = el("div", { class: "radio column" });
      for (const t of m.things().filter((t) => t.traits.includes("openable") && t.id !== id)) {
        box.append(check(t.name, opens.includes(t.id), (v) => ctx.change((x) => x.setList([...p, "pressure", "opens"], v ? [...opens, t.id] : opens.filter((o) => o !== t.id)))));
      }
      out.push(
        el(
          "div",
          { class: "indent" },
          ...numField(ctx, [...p, "pressure", "at_least"], "It takes this much weight", m.get([...p, "pressure", "at_least"]), "The sizes of what's on it, and what's in those. The player weighs their size (1) and what they carry.", "3"),
          el("label", { textContent: "While it's down, it opens" }),
          box,
        ),
      );
    }
    out.push(...boolField(ctx, [...p, "sinks"], "Sinks when overloaded (a raft)", false, "Too much on it, and it would sink: the player is told so, instead of “there's no room”."));
  }
  const people = m.characters();
  const scent = m.get([...p, "scent_of"]);
  out.push(
    ...boolField(ctx, [...p, "floats"], "Floats: held, it keeps the player afloat", false),
    el("label", { textContent: "It smells of" }),
    select(
      [["", "Whoever had it first"], ...people.map((c): [string, string] => [c.id, c.name]), ...m.things().filter((t) => t.id !== id).map((t): [string, string] => [t.id, t.name])],
      typeof scent === "string" ? scent : "",
      (v) => ctx.change((x) => x.set([...p, "scent_of"], v || undefined)),
      "It smells of",
    ),
    help("LET THE DOG SNIFF IT: a character that follows scents leads the way there."),
  );
  return out;
}

/** For a character: follows scents. */
export function characterElements(ctx: InspectorContext, id: string): HTMLElement[] {
  return boolField(ctx, ["characters", id, "sniffs"], "Follows a scent (a dog)", false, "LET THE DOG SNIFF THE GLOVE: it leads the way to whoever the glove belongs to, waiting for the player.");
}

/** For the player: breath under water, in a game with water. */
export function breathField(ctx: InspectorContext): HTMLElement[] {
  const m = ctx.model;
  if (!m.rooms().some((r) => m.get(["rooms", r.id, "floods"]) !== undefined)) return [];
  return [el("h3", { textContent: "Under water" }), ...numField(ctx, ["player", "breath"], "The player can hold their breath for (turns)", m.get(["player", "breath"]), "Then they struggle back the way they came.", "4")];
}
