// The home page: bundled games, and loading your own.
import { fromDrop, fromFileList } from "./files.ts";
import { loadEngine, type Engine, type Files, type Output } from "./engine.ts";
import { endingsText, hashFiles, localGames, savesFor } from "./store.ts";
import { settingsDialog } from "./settings-ui.ts";
import { feedbackButton } from "./feedback.ts";

interface Example {
  id: string;
  title: string;
  author: string | null;
  about: string | null;
  blurb: string;
}

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
settingsDialog($<HTMLDialogElement>("settings"), $("settings-open"));
feedbackButton($("feedback-open"));

function el<K extends keyof HTMLElementTagNameMap>(tag: K, props: Partial<HTMLElementTagNameMap[K]> = {}, ...kids: (Node | string)[]) {
  const e = Object.assign(document.createElement(tag), props);
  e.append(...kids);
  return e;
}

function card(title: string, href: string, about: string | null, blurb: string, game: string, extra: HTMLElement[] = []) {
  const li = el("li");
  li.append(el("a", { className: "title", href, textContent: title }));
  if (about) li.append(el("span", { className: "about", textContent: about }));
  const found = endingsText(game);
  if (found) li.append(el("span", { className: "found", textContent: found }));
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
      const edit = Object.assign(document.createElement("a"), { href: `./editor.html?project=${g.key}`, textContent: "Edit" });
      return card(g.title, `./play.html?local=${g.key}`, `changed ${when}`, "", `local:${g.key}`, [edit, remove]);
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
demo();

// ------------------------------------------------------------- the demo

/**
 * The little game at the top. The engine (about 2 MB) loads the first time
 * someone clicks into it, so visitors who don't, don't pay for it.
 */
function demo() {
  const log = $("demo-log");
  const line = $<HTMLInputElement>("demo-line");
  const form = $<HTMLFormElement>("demo-form");
  let engine: Promise<Engine> | null = null;
  const show = (o: Output) => {
    for (const l of o.lines) log.append(el("p", { className: l.kind === "text" ? "" : l.kind, textContent: l.text }));
    log.scrollTop = log.scrollHeight;
    if (o.ended) {
      form.hidden = true;
      $("demo-form").insertAdjacentElement(
        "afterend",
        el(
          "div",
          { className: "after" },
          el("a", { href: "#examples", textContent: "Play a full game ↓" }),
          el("a", { href: "./editor.html", textContent: "Make your own" }),
        ),
      );
    }
  };
  const start = () =>
    (engine ??= (async () => {
      const [e, yaml] = await Promise.all([loadEngine(new URL("./wasm/", location.href)), import("./demo/game.yaml?raw").then((m) => m.default)]);
      const loaded = e.load({ "game.yaml": yaml });
      // The opening is already on the page; the engine takes over from here.
      if (!loaded.output) log.append(el("p", { className: "error", textContent: "The demo couldn't start." }));
      return e;
    })());
  line.addEventListener("focus", () => void start(), { once: true });
  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    const text = line.value.trim();
    if (!text) return;
    line.value = "";
    log.append(el("p", { className: "cmd", textContent: text }));
    try {
      show((await start()).input(text));
    } catch {
      log.append(el("p", { className: "error", textContent: "The game couldn't load. Try again in a moment." }));
      engine = null;
    }
  });
}
