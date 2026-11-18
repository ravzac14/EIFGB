// The player: loads a game (bundled, or one you loaded), plays it, keeps an
// autosave, and manages save slots and save files.
import { loadEngine, type Diagnostic, type Engine, type Files, type Meta, type Output } from "./engine.ts";
import { localGames, savesFor, type SaveFile, type Slot } from "./store.ts";
import { settingsDialog } from "./settings-ui.ts";
import { fetchGame, resolve } from "./remote.ts";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const log = $("log");
const after = $("after");
const form = $<HTMLFormElement>("prompt");
const line = $<HTMLInputElement>("line");
const savesDialog = $<HTMLDialogElement>("saves");
settingsDialog($<HTMLDialogElement>("settings"), $("settings-open"));

function el<K extends keyof HTMLElementTagNameMap>(tag: K, props: Partial<HTMLElementTagNameMap[K]> = {}, ...kids: (Node | string)[]) {
  const e = Object.assign(document.createElement(tag), props);
  e.append(...kids);
  return e;
}
function button(text: string, onClick: () => void, cls = "") {
  const b = el("button", { type: "button", textContent: text, className: cls });
  b.addEventListener("click", onClick);
  return b;
}
function say(text: string, cls = "") {
  const p = el("p", { textContent: text });
  if (cls) p.className = cls;
  log.append(p);
  return p;
}

// ------------------------------------------------------------ the game

interface Source {
  key: string;
  files: () => Promise<Files>;
  /** For games from a link: where it came from. */
  from?: string;
}

function source(): Source | string {
  const q = new URLSearchParams(location.search);
  const example = q.get("example");
  if (example) {
    if (!/^[\w-]+$/.test(example)) return "That isn't a game we know.";
    return {
      key: `example:${example}`,
      files: async () => {
        const r = await fetch(`./games/${example}.json`);
        if (!r.ok) throw new Error("That isn't a game we know.");
        return r.json();
      },
    };
  }
  const link = q.get("game");
  if (link) {
    let where;
    try {
      where = resolve(link);
    } catch (e) {
      return e instanceof Error ? e.message : String(e);
    }
    const id = where.kind === "gist" ? `gist:${where.id}` : where.url;
    return { key: `url:${id}`, files: () => fetchGame(link), from: where.kind === "gist" ? "a GitHub gist" : new URL(where.url).host };
  }
  const local = q.get("local");
  if (local) {
    const g = localGames.get(local);
    if (!g) return "That game isn't in this browser any more. Load it again from the games page.";
    return { key: `local:${g.key}`, files: async () => g.files };
  }
  return "No game was chosen.";
}

let engine: Engine;
let files: Files;
let meta: Meta;
let saves: ReturnType<typeof savesFor>;
let last: Output | null = null;
/** For a game from a link: where it's from, said above the story. */
let from: string | null = null;
let over = false;
const history: string[] = [];
let back = 0;

function showDiagnostics(ds: Diagnostic[], heading: string) {
  if (!ds.length) return;
  const box = el("section", { className: "diagnostics" });
  box.append(el("p", { textContent: heading }));
  const ul = el("ul");
  for (const d of ds) {
    const li = el("li");
    li.append(el("span", { className: d.severity, textContent: d.severity === "error" ? "Error" : "Warning" }), " ");
    if (d.path) li.append(el("code", { textContent: d.path }), " ");
    li.append(d.message);
    ul.append(li);
  }
  box.append(ul);
  log.append(box);
}

function status(o: Output) {
  const s = o.status;
  const parts = [s.room];
  if (s.max_score !== null || s.score) parts.push(s.max_score !== null ? `Score ${s.score}/${s.max_score}` : `Score ${s.score}`);
  parts.push(`Turn ${s.turns}`);
  $("status").textContent = parts.filter(Boolean).join(" · ");
}

/** Shows an output; returns the first element added, for scrolling. */
function show(o: Output): HTMLElement | null {
  const first = log.lastElementChild;
  for (const l of o.lines) {
    if (l.kind === "choice" && o.options.length) continue;
    say(l.text, l.kind === "text" ? "" : l.kind);
  }
  if (o.options.length) {
    const ol = el("ol", { className: "options" });
    o.options.forEach((text, i) => {
      const b = button("", () => {
        for (const x of ol.querySelectorAll("button")) x.disabled = true;
        send(String(i + 1), text);
      });
      b.append(el("span", { className: "n", textContent: `${i + 1}.` }), text);
      ol.append(el("li", {}, b));
    });
    log.append(ol);
  }
  status(o);
  last = o;
  return (first ? first.nextElementSibling : log.firstElementChild) as HTMLElement | null;
}

function scrollTo(start: HTMLElement | null) {
  if (!start) return;
  // Long replies: keep their start in view. Short ones: keep the prompt in view.
  const tall = log.getBoundingClientRect().bottom - start.getBoundingClientRect().top > window.innerHeight * 0.7;
  if (tall) start.scrollIntoView({ block: "start" });
  else form.scrollIntoView({ block: "end" });
}

function finish(message: string) {
  over = true;
  form.hidden = true;
  saves.clearAuto();
  after.replaceChildren(
    el("span", { className: "dim", textContent: message }),
    button("Play again", () => restart(false), "primary"),
    el("a", { href: "./", textContent: "Back to the games" }),
  );
}

function handle(o: Output) {
  if (o.save) {
    const name = defaultName();
    if (saves.put({ name, at: Date.now(), save: o.save })) say(`Saved as “${name}”. Open Saves to restore or download it.`, "notice");
    else say("There's no room left in this browser to keep that save.", "error");
  }
  if (o.restore_requested) openSaves();
  if (o.ended) return finish(o.ended === "win" ? "You won." : o.ended === "loss" ? "The story is over." : "The end.");
  if (o.quit_requested) return finish("You've stopped playing.");
  saves.setAuto(engine.save());
}

function send(text: string, echo = text) {
  if (over) return;
  say(echo, "cmd");
  const start = log.lastElementChild as HTMLElement;
  const o = engine.input(text);
  show(o);
  handle(o);
  scrollTo(start);
}

function begin(): boolean {
  log.replaceChildren();
  after.replaceChildren();
  over = false;
  const loaded = engine.load(files);
  const errors = loaded.diagnostics.filter((d) => d.severity === "error");
  if (!loaded.output || !loaded.meta) {
    document.title = "Can't play this game";
    say("This game can't be played yet: it has mistakes to fix.", "title");
    showDiagnostics(errors.length ? errors : loaded.diagnostics, "Here's what needs fixing (the same as eifgb check):");
    after.append(el("a", { href: "./", textContent: "Back to the games" }));
    return false;
  }
  meta = loaded.meta;
  if (from) say(`This game comes from ${from}, not from this site.`, "notice");
  showDiagnostics(loaded.diagnostics, "The game has some warnings for its author:");
  show(loaded.output);
  return true;
}

function restart(ask: boolean) {
  if (ask && !confirm("Start again from the beginning? Your autosave will be replaced; save slots are kept.")) return;
  saves.clearAuto();
  if (begin()) {
    form.hidden = false;
    saves.setAuto(engine.save());
    line.focus();
  }
}

// --------------------------------------------------------------- saves

function defaultName() {
  const s = last?.status;
  return s ? `${s.room || "Somewhere"}, turn ${s.turns}` : new Date().toLocaleString();
}

function restoreFrom(save: string) {
  savesDialog.close();
  over = false;
  form.hidden = false;
  after.replaceChildren();
  const start = say("Restoring…", "notice");
  const o = engine.restore(save);
  start.remove();
  const first = show(o);
  saves.setAuto(engine.save());
  scrollTo(first);
  line.focus();
}

function download(slot: Slot) {
  const file: SaveFile = { format: "eifgb-save/1", game: meta.id, title: meta.title, name: slot.name, at: slot.at, save: slot.save };
  const blob = new Blob([JSON.stringify(file)], { type: "application/json" });
  const a = el("a", { href: URL.createObjectURL(blob), download: `${meta.id}-${slot.name.replace(/[^\w-]+/g, "-").replace(/^-|-$/g, "")}.eifgb-save.json` });
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 1000);
}

async function upload(file: File, msg: HTMLElement) {
  try {
    const f = JSON.parse(await file.text()) as Partial<SaveFile>;
    if (f.format !== "eifgb-save/1" || typeof f.save !== "string") throw new Error("not a save file");
    if (f.game !== meta.id && !confirm(`That save is from “${f.title ?? f.game}”, not this game. Try it anyway?`)) return;
    saves.put({ name: f.name ?? file.name, at: f.at ?? Date.now(), save: f.save });
    restoreFrom(f.save);
  } catch {
    msg.textContent = "That file isn't a save from this site.";
  }
}

function openSaves() {
  const render = () => {
    savesDialog.replaceChildren();
    savesDialog.append(button("Done", () => savesDialog.close(), "close"), el("h2", { textContent: "Saves" }));
    savesDialog.append(el("p", { className: "dim", textContent: "Your place is also kept automatically, in this browser." }));
    const name = document.createElement("input");
    name.type = "text";
    name.value = defaultName();
    name.setAttribute("aria-label", "Save name");
    const msg = el("p", { className: "dim", role: "status" });
    const saveNow = button(
      "Save",
      () => {
        if (over) return void (msg.textContent = "The game has ended; there's nothing to save.");
        const n = name.value.trim() || defaultName();
        if (!saves.put({ name: n, at: Date.now(), save: engine.save() })) msg.textContent = "There's no room left in this browser.";
        render();
      },
      "primary",
    );
    savesDialog.append(el("div", { className: "row" }, name, saveNow));
    const slots = saves.slots();
    const ul = el("ul", { className: "slots" });
    if (!slots.length) ul.append(el("li", { className: "dim", textContent: "No saves yet." }));
    for (const s of slots) {
      const li = el("li");
      li.append(el("strong", { textContent: s.name }), el("span", { className: "when", textContent: new Date(s.at).toLocaleString() }));
      li.append(
        el(
          "div",
          { className: "row" },
          button("Restore", () => restoreFrom(s.save)),
          button("Download", () => download(s)),
          button("Delete", () => {
            if (confirm(`Delete the save “${s.name}”?`)) {
              saves.remove(s.name);
              render();
            }
          }),
        ),
      );
      ul.append(li);
    }
    savesDialog.append(ul);
    const fileInput = document.createElement("input");
    fileInput.type = "file";
    fileInput.accept = ".json,application/json";
    fileInput.hidden = true;
    fileInput.addEventListener("change", () => fileInput.files?.[0] && upload(fileInput.files[0], msg));
    savesDialog.append(el("div", { className: "row" }, button("Load a save file…", () => fileInput.click()), fileInput), msg);
  };
  render();
  if (!savesDialog.open) savesDialog.showModal();
}

// ------------------------------------------------------------ starting

form.addEventListener("submit", (e) => {
  e.preventDefault();
  const text = line.value.trim();
  line.value = "";
  if (text) {
    history.push(text);
    back = 0;
  }
  send(text);
});
line.addEventListener("keydown", (e) => {
  if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;
  e.preventDefault();
  back = Math.max(0, Math.min(history.length, back + (e.key === "ArrowUp" ? 1 : -1)));
  line.value = back ? history[history.length - back] : "";
});
// Clicking anywhere in the story (but not selecting text) goes back to typing.
log.addEventListener("click", () => {
  if (!getSelection()?.toString() && !form.hidden) line.focus({ preventScroll: true });
});
$("saves-open").addEventListener("click", openSaves);
$("restart").addEventListener("click", () => restart(true));

async function main() {
  const src = source();
  if (typeof src === "string") {
    log.replaceChildren();
    say(src);
    after.append(el("a", { href: "./", textContent: "Back to the games" }));
    return;
  }
  try {
    [engine, files] = await Promise.all([loadEngine(new URL("./wasm/", location.href)), src.files()]);
  } catch (e) {
    log.replaceChildren();
    say(`The game couldn't be loaded: ${e instanceof Error ? e.message : e}`, "error");
    after.append(el("a", { href: "./", textContent: "Back to the games" }));
    return;
  }
  saves = savesFor(src.key);
  from = src.from ?? null;
  if (!begin()) return;
  document.title = meta.title;
  $("title").textContent = meta.title;
  for (const b of ["saves-open", "restart"]) $<HTMLButtonElement>(b).disabled = false;
  form.hidden = false;

  const auto = saves.auto();
  if (auto) {
    const o = engine.restore(auto);
    if (o.lines.some((l) => l.kind === "notice")) {
      log.replaceChildren();
      say("Welcome back. Here's where you were.", "notice");
      show(o);
      say("(Restart, at the top, begins again.)", "notice");
    } else {
      // The save didn't fit this version of the game: start fresh.
      saves.clearAuto();
      begin();
    }
  }
  saves.setAuto(engine.save());
  line.focus();
}

main();
