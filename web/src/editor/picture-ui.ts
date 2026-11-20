// A picture for a room, object or character: chosen from the author's
// files (made small and kept in the project) or a web address, with words
// for players who can't see it.

import { button, el, field, help, type InspectorContext } from "./ui.ts";

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
    );
    return out;
  }
  const src = ctx.pictureUrl(pic.file);
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
  );
  return out;
}
