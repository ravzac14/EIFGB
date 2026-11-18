// The home page: bundled games, and loading your own.
import { fromDrop, fromFileList } from "./files.ts";
import type { Files } from "./engine.ts";
import { hashFiles, localGames, savesFor } from "./store.ts";
import { settingsDialog } from "./settings-ui.ts";

interface Example {
  id: string;
  title: string;
  author: string | null;
  about: string | null;
  blurb: string;
}

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
settingsDialog($<HTMLDialogElement>("settings"), $("settings-open"));

function el<K extends keyof HTMLElementTagNameMap>(tag: K, props: Partial<HTMLElementTagNameMap[K]> = {}, ...kids: (Node | string)[]) {
  const e = Object.assign(document.createElement(tag), props);
  e.append(...kids);
  return e;
}

function card(title: string, href: string, about: string | null, blurb: string, game: string, extra: HTMLElement[] = []) {
  const li = el("li");
  li.append(el("a", { className: "title", href, textContent: title }));
  if (about) li.append(el("span", { className: "about", textContent: about }));
  if (blurb) li.append(el("p", { textContent: blurb.length > 220 ? blurb.slice(0, 217).trimEnd() + "…" : blurb }));
  const actions = el("div", { className: "actions" });
  const resume = savesFor(game).auto() !== null;
  actions.append(el("a", { href, textContent: resume ? "Continue" : "Play" }));
  actions.append(...extra);
  li.append(actions);
  return li;
}

async function showExamples() {
  const list = $("examples");
  try {
    const games: Example[] = await (await fetch("./games/index.json")).json();
    list.replaceChildren(...games.map((g) => card(g.title, `./play.html?example=${encodeURIComponent(g.id)}`, g.about, g.blurb, `example:${g.id}`)));
  } catch {
    list.replaceChildren(el("li", { className: "dim", textContent: "The games couldn't be loaded." }));
  }
}

function showLocal() {
  const list = $("local");
  list.replaceChildren(
    ...localGames.list().map((g) => {
      const remove = el("button", { type: "button", textContent: "Remove" });
      remove.addEventListener("click", () => {
        if (!confirm(`Remove “${g.title}” and its saves from this browser?`)) return;
        localGames.remove(g.key);
        savesFor(`local:${g.key}`).clearAuto();
        showLocal();
      });
      const when = new Date(g.added).toLocaleDateString();
      return card(g.title, `./play.html?local=${g.key}`, `added ${when}`, "", `local:${g.key}`, [remove]);
    }),
  );
}

async function titleOf(files: Files): Promise<string> {
  try {
    // Loaded only when needed: most visitors never load their own game.
    const { parse } = await import("yaml");
    const t = parse(files["game.yaml"])?.game?.title;
    if (typeof t === "string" && t.trim()) return t.trim();
  } catch {
    /* the play page will show the error */
  }
  return "Untitled game";
}

async function take(files: Promise<Files | string>) {
  const msg = $("drop-msg");
  msg.textContent = "";
  const got = await files.catch((e: unknown) => `That couldn't be read: ${e}`);
  if (typeof got === "string") {
    msg.textContent = got;
    return;
  }
  const key = hashFiles(got);
  if (!localGames.put({ key, title: await titleOf(got), files: got, added: Date.now() })) {
    msg.textContent = "There's no room left in this browser's storage. Remove a game and try again.";
    return;
  }
  location.href = `./play.html?local=${key}`;
}

const folder = $<HTMLInputElement>("folder");
const files = $<HTMLInputElement>("files");
$("pick-folder").addEventListener("click", () => folder.click());
$("pick-files").addEventListener("click", () => files.click());
for (const input of [folder, files]) input.addEventListener("change", () => input.files?.length && take(fromFileList(input.files)));
const drop = $("drop");
drop.addEventListener("dragover", (e) => {
  e.preventDefault();
  drop.classList.add("over");
});
drop.addEventListener("dragleave", () => drop.classList.remove("over"));
drop.addEventListener("drop", (e) => {
  e.preventDefault();
  drop.classList.remove("over");
  if (e.dataTransfer) take(fromDrop(e.dataTransfer));
});

showExamples();
showLocal();
