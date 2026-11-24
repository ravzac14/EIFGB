// The editor's panels for kinds: something written once and used again.
// A room, thing or character can be `like` a kind, and start from its
// fields; the game's panel lists the kinds, and what's like each.

import { button, check, el, help, type InspectorContext } from "./ui.ts";

/** A kind's fields, in a few words: "portable, light source; fuel 20". */
function summary(ctx: InspectorContext, kind: string): string {
  const v = ctx.model.kindFields(kind);
  const parts: string[] = [];
  if (Array.isArray(v.traits) && v.traits.length) parts.push(v.traits.map((t) => String(t).replace(/_/g, " ")).join(", "));
  for (const [k, x] of Object.entries(v)) {
    if (k === "traits") continue;
    const shown = typeof x === "object" && x !== null ? (Array.isArray(x) ? x.join(", ") : Object.keys(x).join(", ")) : String(x);
    parts.push(`${k.replace(/_/g, " ")} ${shown}`);
  }
  return parts.join("; ") || "nothing yet";
}

/** For a room, thing or character: which kinds it's like, and making one of it. */
export function likeSection(ctx: InspectorContext, section: "rooms" | "objects" | "characters", id: string): HTMLElement[] {
  const m = ctx.model;
  const p = [section, id];
  const kinds = m.kindNames();
  const likes = m.list([...p, "like"]);
  const out: HTMLElement[] = [el("h3", { textContent: "Its kind" })];
  if (kinds.length) {
    const box = el("div", { class: "radio column" });
    for (const k of kinds) {
      box.append(
        check(k.replace(/_/g, " "), likes.includes(k), (on) =>
          ctx.change((x) => {
            const next = on ? [...likes, k] : likes.filter((l) => l !== k);
            x.set([...p, "like"], next.length === 1 ? next[0] : next.length ? next : undefined);
          }),
        ),
        help(summary(ctx, k)),
      );
    }
    out.push(el("label", { textContent: "Like" }), box, help("It starts with its kind's fields; its own go on top. Traits add up."));
  }
  const name = el("input", { type: "text", placeholder: section === "characters" ? "guard" : section === "rooms" ? "forest" : "lamp", ariaLabel: "New kind's name" }) as HTMLInputElement;
  out.push(
    el(
      "div",
      { class: "add two" },
      name,
      button("Make a kind of this", () => {
        if (!name.value.trim()) return name.focus();
        ctx.change((x) => x.makeKind(p, name.value));
      }),
    ),
    help("Its fields (all but its name, place, words, description and picture) go to a new kind it's like, for others to be like too."),
  );
  return out;
}

/** The game's panel: the kinds, and what's like each. */
export function kindsSection(ctx: InspectorContext): HTMLElement[] {
  const m = ctx.model;
  const kinds = m.kindNames();
  if (!kinds.length) return [help("Something written once and used again: a guard, a lamp, a stretch of forest. Make one from a room, thing or character's details (Its kind).")];
  const out: HTMLElement[] = [];
  for (const k of kinds) {
    const users = m.usersOfKind(k).map(([s, id]) => (s === "rooms" ? m.room(id)?.name : s === "objects" ? m.thing(id)?.name : m.character(id)?.name) ?? id);
    out.push(
      el("h4", { textContent: k.replace(/_/g, " ") }),
      help(summary(ctx, k)),
      el("p", { class: "dim", textContent: users.length ? `Like it: ${users.join(", ")}.` : "Nothing's like it yet." }),
      el(
        "div",
        { class: "actions" },
        button("Show it in the game file", () => ctx.showInFile(`  ${k}:`), "small"),
        button("Delete it", () => confirm(`Delete the kind “${k}”? What's like it keeps what it gave them.`) && ctx.change((x) => x.deleteKind(k)), "danger small"),
      ),
    );
  }
  out.push(help("Change a kind's fields in the game file: everything like it changes too."));
  return out;
}
