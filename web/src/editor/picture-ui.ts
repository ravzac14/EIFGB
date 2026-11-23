// A picture for a room, object or character: chosen from the author's
// files (made small and kept in the project) or a web address, with words
// for players who can't see it.

import { button, el, field, help, type InspectorContext } from "./ui.ts";
import { sizeText } from "../pictures.ts";

/** How much the game's pictures take, and how full the browser's storage is. */
function spaceNote(ctx: InspectorContext): HTMLElement {
  const pics = ctx.pictures();
  const bytes = pics.reduce((n, p) => n + p.bytes, 0);
  const { used, limit } = ctx.storage();
  const share = Math.min(100, Math.round((used / limit) * 100));
  const unused = pics.filter((p) => !p.usedBy.length);
  return el(
    "p",
    { class: share >= 80 ? "note" : "help" },
    `This game's ${pics.length} picture${pics.length === 1 ? "" : "s"} take ${sizeText(bytes)}. All your games together use ${share}% of the space this browser keeps for them.`,
    share >= 80 ? " It's nearly full: smaller pictures, or fewer, leave room to keep working." : "",
    unused.length ? ` ${unused.length === 1 ? `${unused[0].file} isn't` : `${unused.length} aren't`} used by anything.` : "",
  );
}

/** Use a picture the game already has: the same file, kept once. */
function libraryPicker(ctx: InspectorContext, owner: string[], alt: string, current?: string): HTMLElement[] {
  const pics = ctx.pictures().filter((p) => p.file !== current);
  if (!pics.length) return [];
  const pick = el("select", { ariaLabel: "A picture already in the game" }) as HTMLSelectElement;
  for (const p of pics) {
    const users = p.usedBy.map((u) => u.name).join(", ");
    pick.append(el("option", { value: p.file, textContent: `${p.file.replace(/^pictures\//, "")} (${sizeText(p.bytes)}${users ? `, on ${users}` : ", not used yet"})` }));
  }
  const preview = el("img", { class: "pic-preview small", alt: "" }) as HTMLImageElement;
  const show = () => {
    const src = ctx.pictureUrl(pick.value);
    preview.hidden = !src;
    if (src) preview.src = src;
  };
  pick.addEventListener("change", show);
  show();
  return [
    el("p", { class: "help", textContent: current ? "Or swap in one the game already has (it's kept once, however many places show it):" : "Or use one the game already has, kept once however many places show it:" }),
    el(
      "div",
      { class: "add two" },
      pick,
      button("Use this picture", () => {
        // The words for it travel with it, unless this has its own.
        const theirs = ctx.pictures().find((p) => p.file === pick.value)?.usedBy[0];
        const words = alt || (theirs ? (ctx.model.picture(theirs.owner)?.alt ?? "") : "");
        ctx.change((x) => x.setPicture(owner, pick.value, words));
      }),
    ),
    preview,
  ];
}

export function pictureSection(ctx: InspectorContext, owner: string[], what: "room" | "thing" | "character"): HTMLElement[] {
  const m = ctx.model;
  const pic = m.picture(owner);
  const out: HTMLElement[] = [el("h3", { textContent: "Picture" })];
  const chooser = el("input", { type: "file", accept: "image/png,image/jpeg,image/gif,image/webp,image/svg+xml", hidden: true }) as HTMLInputElement;
  chooser.addEventListener("change", async () => {
    const f = chooser.files?.[0];
    if (!f) return;
    const path = await ctx.addPicture(f, owner[owner.length - 1]);
    if (path) ctx.change((x) => x.setPicture(owner, path, pic?.alt ?? ""));
  });
  const choose = button(pic ? "Choose another picture…" : "Choose a picture…", () => chooser.click());
  if (!pic) {
    out.push(
      help(
        what === "room"
          ? "Shown above the room's description."
          : "Shown when the player examines it: a photograph, a map, a painting, or a clue they have to see for themselves.",
      ),
      chooser,
    );
    const url = el("input", { type: "url", placeholder: "https://…", ariaLabel: "Picture's web address" }) as HTMLInputElement;
    out.push(
      el("div", { class: "row" }, choose),
      el(
        "div",
        { class: "add two" },
        url,
        button("Use this address", () => {
          const v = url.value.trim();
          if (!/^https:\/\//i.test(v)) return url.focus();
          ctx.change((x) => x.setPicture(owner, v, ""));
        }),
      ),
      ...libraryPicker(ctx, owner, ""),
    );
    if (ctx.pictures().length) out.push(spaceNote(ctx));
    return out;
  }
  const src = ctx.pictureUrl(pic.file);
  const others = ctx.pictures().find((p) => p.file === pic.file)?.usedBy.filter((u) => u.owner.join(".") !== owner.join(".")) ?? [];
  out.push(
    src ? el("img", { src, alt: pic.alt || "", class: "pic-preview" }) : el("p", { class: "note", textContent: `The picture ${pic.file} isn't in the game's files.` }),
    ...field("What it shows", pic.alt, (v) => ctx.change((x) => x.setPicture(owner, pic.file, v), { coalesce: `alt:${owner.join(".")}`, redraw: false }), {
      multiline: true,
      placeholder: "A faded photograph of three children on a beach. One holds a red kite.",
      help: "For players who can't see it (screen readers, read aloud). Say what matters, the clue too.",
    }),
    chooser,
    el(
      "div",
      { class: "row" },
      choose,
      button("Remove the picture", () => {
        ctx.change((x) => x.setPicture(owner, null, ""));
        ctx.dropPicture(pic.file);
      }, "danger"),
    ),
    ...libraryPicker(ctx, owner, pic.alt, pic.file),
  );
  if (others.length) out.splice(2, 0, help(`The same picture is on ${others.map((o) => o.name).join(", ")}. Choosing another one here changes only this.`));
  if (ctx.pictures().length) out.push(spaceNote(ctx));
  return out;
}
