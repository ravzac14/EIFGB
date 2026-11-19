// The player: loads a game (bundled, or one you loaded), plays it, keeps an
// autosave, and manages save slots and save files.
import { loadEngine, type Diagnostic, type Engine, type Files, type Meta, type Output } from "./engine.ts";
import { endingsFor, endingsText, localGames, mapPanel, savesFor, setGameLook, settings, type SaveFile, type Slot } from "./store.ts";
import { authorPositions, drawMap } from "./player-map.ts";
import { settingsDialog } from "./settings-ui.ts";
import { feedbackButton, feedbackContext } from "./feedback.ts";
import { fetchGame, resolve } from "./remote.ts";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const log = $("log");
const after = $("after");
const form = $<HTMLFormElement>("prompt");
const line = $<HTMLInputElement>("line");
const savesDialog = $<HTMLDialogElement>("saves");
settingsDialog($<HTMLDialogElement>("settings"), $("settings-open"));
feedbackButton($("feedback-open"));
feedbackContext({
  info: () => ({
    game: meta ? `${meta.title}${meta.author ? ` by ${meta.author}` : ""}` : "not loaded",
    from: from ?? (gameKey.split(":")[0] || ""),
    where: last ? `${last.status.room || "?"}, turn ${last.status.turns}${last.status.max_score !== null ? `, score ${last.status.score}/${last.status.max_score}` : ""}` : "",
    state: over ? "the game has ended" : last?.options.length ? "choosing from a list" : "playing",
    map: mapBox.hidden ? "closed" : "open",
  }),
  versions: () => (meta ? { game: `${meta.id} ${meta.version}`, "game built with": `stdlib v${meta.stdlib ?? "?"}, IR ${meta.ir ?? "?"}, engine ${meta.engine ?? "?"}` } : {}),
  // The last lines of the story, commands marked with ">".
  log: () =>
    [...log.children]
      .filter((e) => !e.classList.contains("diagnostics"))
      .map((e) => (e.classList.contains("cmd") ? `> ${e.textContent}` : (e.textContent ?? "")).trim().slice(0, 300))
      .filter(Boolean)
      .slice(-20)
      .join("\n"),
});

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
/** The game's key in this browser's storage (its saves and endings). */
let gameKey = "";
/** The sharing guide's safety section. */
const SAFETY = "./docs/sharing.html#is-it-safe-to-play-other-peoples-games";
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
  const typed = settings.get().reveal === "typewriter";
  // Choices are numbered lines; the player types the number.
  for (const l of o.lines) {
    const p = say(l.text, l.kind === "text" ? "" : l.kind);
    if (typed) typeOut(p);
  }
  status(o);
  last = o;
  refreshMap();
  return (first ? first.nextElementSibling : log.firstElementChild) as HTMLElement | null;
}

// ------------------------------------------------------- typewriter

/*
 * "Text appears: letter by letter" in Text and colours. A line's whole text
 * is on the page from the start, the part not typed yet see-through, so the
 * layout doesn't move, screen readers read it all at once, and the
 * transcript is complete. Any key, click or tap shows everything at once.
 */
const LETTERS_PER_SECOND = 90;
const typing: { p: HTMLElement; text: string; shown: Text; rest: HTMLElement }[] = [];
let typingFrame = 0;
let typedAt = 0;
let owed = 0;

function typeOut(p: HTMLElement) {
  const text = p.textContent ?? "";
  if (!text) return;
  const shown = document.createTextNode("");
  const rest = el("span", { className: "untyped", textContent: text });
  p.replaceChildren(shown, rest);
  typing.push({ p, text, shown, rest });
  if (!typingFrame) {
    typedAt = performance.now();
    owed = 0;
    typingFrame = requestAnimationFrame(typeStep);
  }
}

function typeStep(now: number) {
  owed += ((now - typedAt) / 1000) * LETTERS_PER_SECOND;
  typedAt = now;
  while (typing.length && owed >= 1) {
    const t = typing[0];
    const had = t.shown.length;
    const n = Math.min(t.text.length, had + Math.floor(owed));
    owed -= n - had;
    t.shown.data = t.text.slice(0, n);
    t.rest.textContent = t.text.slice(n);
    if (n >= t.text.length) {
      t.p.textContent = t.text;
      typing.shift();
    }
  }
  typingFrame = typing.length ? requestAnimationFrame(typeStep) : 0;
}

/** Shows everything still being typed. */
function skipTyping() {
  for (const t of typing) t.p.textContent = t.text;
  typing.length = 0;
  cancelAnimationFrame(typingFrame);
  typingFrame = 0;
}
for (const type of ["keydown", "pointerdown"]) document.addEventListener(type, () => typing.length && skipTyping(), true);

/**
 * Scrolls down to show what's new: as far as the bottom of the page, but
 * never so far that `start` (the command, or the first new line) goes up
 * under the header, so a long reply can be read from its beginning.
 */
function scrollTo(start: HTMLElement | null) {
  if (!start) return;
  const header = document.querySelector<HTMLElement>("header.bar")?.offsetHeight ?? 0;
  const top = start.getBoundingClientRect().top + window.scrollY - header - 12;
  const bottom = document.documentElement.scrollHeight - window.innerHeight;
  const smooth = !matchMedia("(prefers-reduced-motion: reduce)").matches;
  window.scrollTo({ top: Math.max(0, Math.min(top, bottom)), behavior: smooth ? "smooth" : "auto" });
}

// ------------------------------------------------------------- the map

const mapBox = $("map");
const mapBody = $("map-body");
const mapButton = $<HTMLButtonElement>("map-open");
// The narrow-screen map drops down from just under the header.
const bar = document.querySelector<HTMLElement>("header.bar")!;
new ResizeObserver(() => document.documentElement.style.setProperty("--bar-h", `${bar.offsetHeight}px`)).observe(bar);
let author: ReturnType<typeof authorPositions> = new Map();

function refreshMap() {
  if (mapBox.hidden || !engine) return;
  const m = engine.map();
  if ("error" in m) return;
  drawMap(mapBody, m, author);
}

function showMap(open: boolean, remember = true) {
  mapBox.hidden = !open;
  document.body.classList.toggle("with-map", open);
  mapButton.setAttribute("aria-pressed", String(open));
  if (remember) mapPanel.set(open);
  refreshMap();
}

/** The story so far, as plain text: what's on the page, commands marked with ">". */
function transcriptText(): string {
  const lines: string[] = [];
  const head = [meta?.title, meta?.author ? `by ${meta.author}` : ""].filter(Boolean).join(", ");
  if (head) lines.push(head, `Played ${new Date().toLocaleString()}`, "");
  for (const node of log.children) {
    const e = node as HTMLElement;
    if (e.classList.contains("diagnostics")) continue;
    const text = e.textContent?.trim() ?? "";
    if (!text) continue;
    if (e.classList.contains("cmd")) lines.push("", `> ${text}`);
    else lines.push(text);
  }
  return lines.join("\n").replace(/\n{3,}/g, "\n\n").trim() + "\n";
}

function downloadTranscript() {
  const blob = new Blob([transcriptText()], { type: "text/plain;charset=utf-8" });
  const a = el("a", { href: URL.createObjectURL(blob), download: `${meta.id}-transcript.txt` });
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 1000);
}

/**
 * The game has ended (or the player quit). The prompt stays: the game says
 * UNDO, RESTORE or RESTART still work, and they do.
 */
function finish(message: string) {
  over = true;
  saves.clearAuto();
  after.replaceChildren(
    el("span", { className: "dim", textContent: message }),
    button("Play again", () => restart(false), "primary"),
    button("Download the transcript", downloadTranscript),
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
  // Back in the story after an ending (UNDO, RESTART or RESTORE): carry on.
  if (over && !o.ended && !o.quit_requested) {
    over = false;
    after.replaceChildren();
  }
  if (o.ended) {
    if (o.ending) endingsFor(gameKey).record(o.ending, meta.endings ?? [o.ending]);
    const found = endingsText(gameKey);
    return finish(`${o.ended === "win" ? "You won." : o.ended === "loss" ? "The story is over." : "The end."}${found ? ` ${found}` : ""}`);
  }
  if (o.quit_requested) return finish("You've stopped playing.");
  saves.setAuto(engine.save());
}

function send(text: string, echo = text) {
  skipTyping();
  say(echo, "cmd");
  const start = log.lastElementChild as HTMLElement;
  const o = engine.input(text);
  show(o);
  handle(o);
  scrollTo(start);
}

function begin(): boolean {
  skipTyping();
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
  setGameLook(meta.presentation);
  if (from) {
    const p = say(`This game comes from ${from}, not from this site. `, "notice");
    p.append(el("a", { href: SAFETY, target: "_blank", rel: "noopener", textContent: "Is that safe?" }));
  }
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
    savesDialog.append(
      el("h3", { textContent: "Transcript" }),
      el("p", { className: "dim", textContent: "Everything on the page since you opened it, as a text file." }),
      el("div", { className: "row" }, button("Download the transcript", downloadTranscript)),
    );
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
mapButton.addEventListener("click", () => showMap(mapBox.hidden));
$("map-close").addEventListener("click", () => {
  showMap(false);
  mapButton.focus();
});
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
  gameKey = src.key;
  // The editor's Play button: always from the beginning.
  if (new URLSearchParams(location.search).has("fresh")) {
    saves.clearAuto();
    window.history.replaceState(null, "", location.href.replace(/[?&]fresh=1/, ""));
  }
  from = src.from ?? null;
  author = authorPositions(files["map.yaml"]);
  if (!begin()) return;
  document.title = meta.title;
  $("title").textContent = meta.title;
  for (const b of ["map-open", "saves-open", "restart"]) $<HTMLButtonElement>(b).disabled = false;
  form.hidden = false;

  const auto = saves.auto();
  if (auto) {
    const o = engine.restore(auto);
    if (o.lines.some((l) => l.kind === "notice")) {
      log.replaceChildren();
      say("Welcome back. Here's where you were.", "notice");
      show(o);
      say("(Restart, at the top, begins again.)", "notice");
      scrollTo(log.lastElementChild as HTMLElement);
    } else {
      // The save didn't fit this version of the game: start fresh.
      saves.clearAuto();
      begin();
    }
  }
  saves.setAuto(engine.save());
  // The map starts open on wide screens, until the player says otherwise.
  showMap(mapPanel.open() ?? matchMedia("(min-width: 70rem)").matches, false);
  line.focus({ preventScroll: true });
}

main();
