// The editor's Sound fields: a room's own sound (played over and over
// while the player's there) and a timer's (played once when it goes off).
// A sound is a file kept with the game, under sounds/, or a web address.

import { button, el, help, type InspectorContext } from "./ui.ts";

/** `sound_file` on a room or timer: choose a file, use an address, hear it, or take it off. */
export function soundSection(ctx: InspectorContext, owner: string[], what: "room" | "timer"): HTMLElement[] {
  const m = ctx.model;
  const path = [...owner, "sound_file"];
  const file = typeof m.get(path) === "string" ? (m.get(path) as string) : "";
  const out: HTMLElement[] = [el("label", { textContent: what === "room" ? "Its own sound" : "Plays a sound" })];
  const chooser = el("input", { type: "file", accept: "audio/*,.mp3,.ogg,.oga,.wav,.m4a,.opus", hidden: true }) as HTMLInputElement;
  chooser.addEventListener("change", async () => {
    const f = chooser.files?.[0];
    if (!f) return;
    const p = await ctx.addSound(f, owner[owner.length - 1]);
    if (p) ctx.change((x) => x.set(path, p));
  });
  const about =
    what === "room"
      ? "Played over and over while the player's here: rain, a fire, the sea. Players hear it only with Sound turned on, so say it in words too."
      : "Played once when it goes off, where it's heard. Players hear it only with Sound turned on.";
  if (!file) {
    const url = el("input", { type: "url", placeholder: "https://…", ariaLabel: "Sound's web address" }) as HTMLInputElement;
    const kept = ctx.sounds();
    const pick = el("select", { ariaLabel: "A sound already in the game" }) as HTMLSelectElement;
    pick.append(el("option", { value: "", textContent: "A sound already in the game…" }), ...kept.map((s) => el("option", { value: s, textContent: s })));
    pick.addEventListener("change", () => pick.value && ctx.change((x) => x.set(path, pick.value)));
    out.push(
      help(about),
      chooser,
      el("div", { class: "row" }, button("Choose a sound file…", () => chooser.click())),
      el("div", { class: "add two" }, url, button("Use this address", () => {
        const v = url.value.trim();
        if (!/^https:\/\//i.test(v)) return url.focus();
        ctx.change((x) => x.set(path, v));
      })),
    );
    if (kept.length) out.push(pick);
    return out;
  }
  const src = ctx.soundUrl(file);
  out.push(
    src ? el("audio", { controls: true, src, preload: "none", class: "sound-preview", ariaLabel: `Hear ${file}` }) : help(`The sound ${file} isn't in the game's files.`),
    help(`${file}. ${about}`),
    chooser,
    el(
      "div",
      { class: "row" },
      button("Choose another…", () => chooser.click()),
      button("Remove the sound", () => {
        ctx.change((x) => x.set(path, undefined));
        ctx.dropPicture(file);
      }, "danger"),
    ),
  );
  return out;
}
