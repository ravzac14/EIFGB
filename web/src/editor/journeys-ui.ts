// The editor's panels for getting about and looking back: boats and
// horses (and the ways only they can go), crowds, characters who read
// notes, flashbacks the player plays through, and the epilogue.

import type { Direction } from "./model.ts";
import { boolField, button, check, el, field, help, numField, select, type InspectorContext } from "./ui.ts";

const text = (ctx: InspectorContext, path: (string | number)[], label: string, opts: { multiline?: boolean; placeholder?: string; help?: string } = {}) =>
  field(label, String(ctx.model.get(path) ?? ""), (v) => ctx.change((x) => x.set(path, v.trim() ? v : undefined), { coalesce: path.join("."), redraw: false }), opts);

/** The game's flashbacks, by key. */
const flashbackKeys = (ctx: InspectorContext) => ctx.model.pairs(["flashbacks"]).map(([k]) => k);

/** For an object: a boat or cart, a memory it brings back, and what it is now. */
export function thingJourneys(ctx: InspectorContext, id: string, traits: string[]): HTMLElement[] {
  const m = ctx.model;
  const p = ["objects", id];
  const out: HTMLElement[] = [];
  if (traits.includes("enterable")) {
    out.push(...boolField(ctx, [...p, "vehicle"], "A boat or a cart: it carries the player", false, "Get in, then go a way that's only “by” it (on the way's details): ROW EAST. Other ways, the player gets out and walks."));
  }
  const keys = flashbackKeys(ctx);
  if (keys.length) {
    const fb = m.get([...p, "flashback"]);
    const now = m.get([...p, "now"]);
    out.push(
      el("label", { textContent: "Looking at it brings back" }),
      select([["", "No memory"], ...keys.map((k): [string, string] => [k, k.replace(/_/g, " ")])], typeof fb === "string" ? fb : "", (v) => ctx.change((x) => x.set([...p, "flashback"], v || undefined)), "Looking at it brings back"),
      help("EXAMINE or READ it, the first time, and the memory plays."),
      el("label", { textContent: "In a memory, this is what's now" }),
      select([["", "Nothing"], ...m.things().filter((t) => t.id !== id).map((t): [string, string] => [t.id, t.name])], typeof now === "string" ? now : "", (v) => ctx.change((x) => x.set([...p, "now"], v || undefined)), "In a memory, this is what's now"),
      help("Left in a past room, what it is now is found in that room's present one afterwards. Leave that unplaced."),
    );
  }
  return out;
}

/** For a room: crowded, and (with flashbacks) the room it is now. */
export function roomJourneys(ctx: InspectorContext, id: string): HTMLElement[] {
  const m = ctx.model;
  const p = ["rooms", id];
  const crowd = m.get([...p, "crowd"]);
  const mode = crowd === true ? "always" : typeof crowd === "string" ? "hours" : "";
  const out: HTMLElement[] = [
    el("label", { textContent: "Crowded" }),
    select(
      [["", "Never"], ["always", "Always"], ...(m.get(["clock"]) !== undefined ? [["hours", "At certain hours"] as [string, string]] : [])],
      mode,
      (v) => ctx.change((x) => x.set([...p, "crowd"], v === "always" ? true : v === "hours" ? "8:00-18:00" : undefined)),
      "Crowded",
    ),
  ];
  if (mode === "hours") out.push(...text(ctx, [...p, "crowd"], "Crowded between", { placeholder: "8:00-18:00" }));
  if (mode) out.push(help("In a crowd, nobody who'd go for the player can pick them out, until they make a scene; thieves go unnoticed (mostly); and nobody leaves tracks."));
  if (flashbackKeys(ctx).length) {
    const now = m.get([...p, "now"]);
    out.push(
      el("label", { textContent: "In a memory, this room is now" }),
      select([["", "Not a room in the past"], ...m.rooms().filter((r) => r.id !== id).map((r): [string, string] => [r.id, r.name])], typeof now === "string" ? now : "", (v) => ctx.change((x) => x.set([...p, "now"], v || undefined)), "In a memory, this room is now"),
    );
  }
  return out;
}

/** For a character: reads notes; can be ridden. */
export function characterJourneys(ctx: InspectorContext, id: string): HTMLElement[] {
  const m = ctx.model;
  const p = ["characters", id];
  const talks = m.get([...p, "talk"]) !== undefined || ["merchant", "conversationalist"].includes(String(m.get([...p, "behavior"]) ?? ""));
  const mount = m.bool([...p, "mount"], false);
  return [
    ...boolField(ctx, [...p, "reads"], "Reads notes", talks, "Given one, or finding one in their room: if they believe it, they go where it says, and wait there a while. People with something to say do, unless you say not."),
    ...boolField(ctx, [...p, "mount"], "Can be ridden (a horse)", false, "RIDE IT, if it's tame or friendly: it goes where the player goes, and GO TO takes half the turns."),
    ...(mount ? numField(ctx, [...p, "stamina"], "Tires after (moves)", m.get([...p, "stamina"]), "Then it needs feeding something it likes. Empty: never.", "never") : []),
  ];
}

/** For an exit: only in a boat, or on a horse. */
export function exitBy(ctx: InspectorContext, room: string, dir: Direction, by: string[]): HTMLElement[] {
  const m = ctx.model;
  const can = [
    ...m.things().filter((t) => m.bool(["objects", t.id, "vehicle"], false)).map((t): [string, string] => [t.id, t.name]),
    ...m.characters().filter((c) => m.bool(["characters", c.id, "mount"], false)).map((c): [string, string] => [c.id, c.name]),
  ];
  const out: HTMLElement[] = [el("h3", { textContent: "Only by" })];
  if (!can.length) return [...out, help("Make an object a boat or a cart, or a character something to ride, to have ways only they can go.")];
  const box = el("div", { class: "radio column" });
  for (const [v, name] of can) box.append(check(name, by.includes(v), (on) => ctx.change((x) => x.setBy(room, dir, on ? [...by, v] : by.filter((b) => b !== v)))));
  return [...out, box, help("A river only a boat goes along, a long road only a horse can manage. On foot: “You can't go that way without a rowing boat.”")];
}

/** The epilogue: lines after an ending, by what the player did. */
export function epilogueSection(ctx: InspectorContext): HTMLElement[] {
  const m = ctx.model;
  const raw = m.get(["epilogue"]) as { toJSON?: () => unknown } | undefined;
  const lines = (raw?.toJSON?.() ?? []) as (string | { text?: string; when?: string; endings?: string | string[] })[];
  const endings = m.ids("endings");
  const out: HTMLElement[] = [
    help("Said after the ending, each when its condition holds: what became of the dog, the promise kept, the baker who never forgave you."),
  ];
  lines.forEach((l, i) => {
    const p = ["epilogue", i];
    const full = typeof l === "object" && l !== null;
    const only = full && l.endings ? (Array.isArray(l.endings) ? l.endings : [l.endings]) : [];
    const write = (patch: Record<string, unknown>) =>
      ctx.change((x) => {
        const cur = (x.get(p) as { toJSON?: () => unknown } | string | undefined) ?? "";
        const now = typeof cur === "string" ? { text: cur } : ((cur.toJSON?.() ?? {}) as Record<string, unknown>);
        const next = Object.fromEntries(Object.entries({ ...now, ...patch }).filter(([, v]) => v !== undefined && v !== "" && !(Array.isArray(v) && !v.length)));
        x.set(p, Object.keys(next).length === 1 && "text" in next ? next.text : next);
      }, { coalesce: `epilogue.${i}`, redraw: false });
    const box = el("div", { class: "radio" });
    for (const e of endings) {
      box.append(check(e.replace(/_/g, " "), only.includes(e), (on) => ctx.change((x) => {
        const cur = x.get(p) as { toJSON?: () => unknown } | string;
        const now = typeof cur === "string" ? { text: cur } : ((cur.toJSON?.() ?? {}) as Record<string, unknown>);
        const list = on ? [...only, e] : only.filter((o) => o !== e);
        x.set(p, { ...now, endings: list.length ? list : undefined });
      })));
    }
    out.push(
      el("h4", {}, `Line ${i + 1} `, button("✕", () => ctx.change((x) => {
        const all = ((x.get(["epilogue"]) as { toJSON?: () => unknown }).toJSON?.() ?? []) as unknown[];
        all.splice(i, 1);
        x.set(["epilogue"], all.length ? all : undefined);
      }), "small")),
      ...field("It says", full ? String(l.text ?? "") : String(l), (v) => write({ text: v }), { multiline: true, placeholder: "Rex lived to be very old." }),
      ...field("When (Lua)", full ? String(l.when ?? "") : "", (v) => write({ when: v.trim() || undefined }), { placeholder: 'world.dog.name == "Rex"', help: "Empty: always." }),
      ...(endings.length ? [el("label", { textContent: "Only after" }), box] : []),
    );
  });
  out.push(
    el("div", { class: "actions" }, button("Add a line", () => ctx.change((x) => {
      const all = ((x.get(["epilogue"]) as { toJSON?: () => unknown } | undefined)?.toJSON?.() ?? []) as unknown[];
      if (x.get(["epilogue"]) === undefined) x.addSection("epilogue", ["And so it ended."], ["endings", "goals", "hints"]);
      else x.set(["epilogue"], [...all, "And so it ended."]);
    }))),
  );
  return out;
}

/** The game's flashbacks: where each starts, what the player has, how long it lasts. */
export function flashbacksSection(ctx: InspectorContext): HTMLElement[] {
  const m = ctx.model;
  const keys = flashbackKeys(ctx);
  const out: HTMLElement[] = [
    help("Memories the player plays through: what they carry is set aside, and they're somewhere in the past for a while. A thing left in a past room turns up in the present (set what each past room and thing is “now” in their details)."),
  ];
  for (const k of keys) {
    const p = ["flashbacks", k];
    const carries = m.list([...p, "carries"]);
    const box = el("div", { class: "radio column" });
    for (const t of m.things()) box.append(check(t.name, carries.includes(t.id), (on) => ctx.change((x) => x.setList([...p, "carries"], on ? [...carries, t.id] : carries.filter((c) => c !== t.id)))));
    out.push(
      el("h4", {}, k.replace(/_/g, " "), " ", button("✕", () => ctx.change((x) => x.set(p, undefined)), "small")),
      el("label", { textContent: "It starts in" }),
      select(m.rooms().map((r): [string, string] => [r.id, r.name]), String(m.get([...p, "at"]) ?? ""), (v) => ctx.change((x) => x.set([...p, "at"], v)), "It starts in"),
      el("label", { textContent: "In it, the player has" }),
      box,
      ...text(ctx, [...p, "begins"], "It begins with", { multiline: true, placeholder: "You're small again, in the garden." }),
      ...numField(ctx, [...p, "turns"], "It lasts (turns)", m.get([...p, "turns"]), "Or until a condition holds, below. Scripts: flashback(\"" + k + "\"), end_flashback().", ""),
      ...text(ctx, [...p, "until"], "Or until (Lua)", { placeholder: "world.ring_then.location == world.garden_then" }),
      ...text(ctx, [...p, "ends"], "It ends with", { multiline: true, placeholder: "The memory fades." }),
    );
  }
  const name = el("input", { type: "text", placeholder: "childhood", ariaLabel: "New flashback" }) as HTMLInputElement;
  out.push(
    el(
      "div",
      { class: "add two" },
      name,
      button("Add a flashback", () => {
        const k = name.value.trim().toLowerCase().replace(/[^a-z0-9_]+/g, "_").replace(/^_+|_+$/g, "");
        const at = m.rooms()[0]?.id;
        if (!k || keys.includes(k) || !at) return;
        ctx.change((x) => (x.get(["flashbacks"]) === undefined ? x.addSection("flashbacks", { [k]: { at, turns: 5 } }, ["scenes", "goals", "endings"]) : x.set(["flashbacks", k], { at, turns: 5 })));
      }),
    ),
  );
  return out;
}
