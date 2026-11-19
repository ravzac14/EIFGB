// The editor page: choosing a project, then the map, the details panel and
// the drawer (problems, and the game file). Projects are kept in the
// browser with the games you've loaded, so they're on the home page too.

import { zipSync, strToU8 } from "fflate";
import { loadEngine, type Diagnostic, type Engine, type Files } from "../engine.ts";
import { fromFileList } from "../files.ts";
import { blueprint, localGames, type LocalGame } from "../store.ts";
import { settingsDialog } from "../settings-ui.ts";
import { feedbackButton, feedbackContext } from "../feedback.ts";
import { Model, NEW_GAME, slug, type Direction } from "./model.ts";
import { MAP_FILE, besideRoom, layout, readMap, writeMap, type Point, type Positions } from "./layout.ts";
import { MapView, type Selection } from "./map.ts";
import { renderInspector } from "./inspector.ts";
import { TestPlay } from "./testplay.ts";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
settingsDialog($<HTMLDialogElement>("settings"), $("settings-open"));
feedbackButton($("feedback-open"));
feedbackContext({
  info: () => ({
    editing: model ? `${model.title()} (${model.roomIds().length} rooms)` : "nothing yet (the start screen)",
    selected: selection ? `${selection.kind} ${"id" in selection ? selection.id : `${selection.room} ${selection.dir}`}` : "the game",
    problems: problems.length ? problems.slice(0, 5).map((d) => `${d.severity} ${d.path}: ${d.message}`).join(" | ") : "none",
    files: project ? Object.keys(project.files).join(", ") : "",
  }),
});

function el<K extends keyof HTMLElementTagNameMap>(tag: K, props: Partial<HTMLElementTagNameMap[K]> = {}, ...kids: (Node | string)[]) {
  const e = Object.assign(document.createElement(tag), props);
  e.append(...kids);
  return e;
}
const button = (text: string, onClick: () => void, cls = "") => {
  const b = el("button", { type: "button", textContent: text, className: cls });
  b.addEventListener("click", onClick);
  return b;
};

// --------------------------------------------------------- the project

let project: LocalGame;
let model: Model | null = null;
let positions: Positions = new Map();
let selection: Selection = null;
let problems: Diagnostic[] = [];
let mapView: MapView;

// Test play, in the drawer.
const testPlay = new TestPlay($("panel-play"), {
  engine: () => (engine ??= loadEngine(new URL("./wasm/", location.href))),
  files: () => {
    save();
    return { ...project.files };
  },
  selectedRoom: () => {
    const r = selection?.kind === "room" && model ? model.room(selection.id) : null;
    return r && selection?.kind === "room" ? { id: selection.id, name: r.name } : null;
  },
  markHere: (room) => mapView?.setHere(room),
  showProblems: () => showTab("problems"),
});

/** Every file of the project at one moment: what undo goes back to. */
interface Snapshot {
  files: Files;
}
const undo: Snapshot[] = [];
const redo: Snapshot[] = [];
let lastCoalesce: { key: string; at: number } | null = null;

const snapshot = (): Snapshot => ({
  files: { ...project.files, "game.yaml": model ? model.toString() : project.files["game.yaml"], ...(model ? { [MAP_FILE]: mapText() } : {}) },
});
const sameFiles = (a: Files, b: Files) => {
  const keys = new Set([...Object.keys(a), ...Object.keys(b)]);
  return [...keys].every((k) => a[k] === b[k]);
};
/** The standard verbs, for the script picker (loaded at start). */
let verbs: { id: string; example: string }[] = [];
/** The file open in the Files tab. */
let currentFile = "game.yaml";
const mapText = () => (model ? writeMap(positions, model.roomIds()) : (project.files[MAP_FILE] ?? ""));

function newKey() {
  return `e${Date.now().toString(36)}${Math.floor(Math.random() * 1296).toString(36)}`;
}

function open(game: LocalGame) {
  project = game;
  history.replaceState(null, "", `?project=${encodeURIComponent(game.key)}`);
  $("start").hidden = true;
  $("workspace").hidden = false;
  $("tools").hidden = false;
  undo.length = 0;
  redo.length = 0;
  load(game.files["game.yaml"] ?? "", game.files[MAP_FILE]);
  requestAnimationFrame(() => mapView.fit());
}

/** (Re)reads the file; a file with a YAML mistake opens on the game file tab. */
function load(yaml: string, map?: string) {
  try {
    model = new Model(yaml);
    positions = layout(model, readMap(map));
    $("yaml-msg").textContent = "";
  } catch (e) {
    model = null;
    $("yaml-msg").textContent = `The file has a mistake, so the map can't be shown: ${e instanceof Error ? e.message : e}`;
    showTab("yaml");
  }
  if (selection?.kind === "room" && !model?.room(selection.id)) selection = null;
  if (selection?.kind === "exit" && !model?.exit(selection.room, selection.dir)) selection = null;
  if (selection?.kind === "thing" && !model?.thing(selection.id)) selection = null;
  if (selection?.kind === "character" && !model?.character(selection.id)) selection = null;
  if (selection?.kind === "faction" && !model?.faction(selection.id)) selection = null;
  for (const [kind, section] of [["ending", "endings"], ["timer", "timers"], ["choice", "choices"], ["verb", "verbs"]] as const) {
    if (selection?.kind === kind && !model?.ids(section).includes(selection.id)) selection = null;
  }
  redraw();
  syncYaml(yaml);
  scheduleCheck();
}

function save() {
  const files: Files = { ...project.files, "game.yaml": model ? model.toString() : project.files["game.yaml"] };
  if (model) files[MAP_FILE] = mapText();
  project = { ...project, files, title: model?.title() ?? project.title, added: Date.now() };
  const ok = localGames.put(project);
  $("saved").textContent = ok ? "Saved in this browser" : "Couldn't save: this browser's storage is full";
  $("saved").classList.toggle("err", !ok);
}

/** Every edit comes through here: one undo step, a save, a redraw and a check. */
function change(fn: (m: Model) => void, opts: { coalesce?: string; redraw?: boolean } = {}) {
  if (!model) return;
  const before = snapshot();
  fn(model);
  const after = snapshot();
  if (!sameFiles(before.files, after.files)) {
    const now = Date.now();
    const merge = opts.coalesce && lastCoalesce?.key === opts.coalesce && now - lastCoalesce.at < 2000;
    if (!merge) undo.push(before);
    lastCoalesce = opts.coalesce ? { key: opts.coalesce, at: now } : null;
    redo.length = 0;
    // New rooms need places on the map.
    positions = layout(model, positions);
    save();
    syncYaml(after.files["game.yaml"]);
    scheduleCheck();
  }
  if (opts.redraw === false) drawMap();
  else redraw();
}

function restore(s: Snapshot) {
  project = { ...project, files: { ...s.files } };
  load(s.files["game.yaml"] ?? "", s.files[MAP_FILE]);
  if (currentFile !== "game.yaml") openFile(project.files[currentFile] === undefined ? "game.yaml" : currentFile);
  save();
}

function doUndo() {
  const s = undo.pop();
  if (!s) return;
  redo.push(snapshot());
  restore(s);
}
function doRedo() {
  const s = redo.pop();
  if (!s) return;
  undo.push(snapshot());
  restore(s);
}

// ----------------------------------------------------------- drawing

function badRooms(): Set<string> {
  const out = new Set<string>();
  for (const d of problems) {
    if (d.severity !== "error") continue;
    const [section, id] = d.path.split(/[.[]/);
    if (section === "rooms" && id) out.add(id);
  }
  return out;
}

function drawMap() {
  if (model) mapView.render(model, positions, selection, badRooms());
  $<HTMLButtonElement>("undo").disabled = !undo.length;
  $<HTMLButtonElement>("redo").disabled = !redo.length;
  $("title").textContent = model?.title() ?? project.title;
  document.title = `${model?.title() ?? project.title} · Editor`;
  $("map-hint").textContent =
    selection?.kind === "room"
      ? "Click a + to add a room that way, or drag it onto another room to connect them."
      : model && model.roomIds().length < 2
        ? "Click a room, then one of the + around it to add a room next door."
        : "";
}

function redraw() {
  drawMap();
  testPlay.update();
  if (!model) {
    $("inspector").replaceChildren(el("p", { className: "dim", textContent: "Fix the mistake in the game file to see the map." }));
    return;
  }
  renderInspector($("inspector"), {
    model,
    selection,
    change,
    select,
    addRoomBeside: (from, dir) => addBeside(from, dir),
    focusName,
    notice: (text) => {
      $("saved").textContent = text;
      $("saved").classList.add("err");
    },
    deleteRoom: (id) => {
      selection = null;
      change((m) => {
        m.deleteRoom(id);
        positions.delete(id);
      });
    },
    showInFile,
    problems,
    verbs,
    openFile,
    addScriptFile,
    roomColour: (id) => positions.get(id)?.colour,
    setRoomColour: (id, colour) =>
      change(() => {
        const p = positions.get(id);
        if (!p) return;
        const { colour: _old, ...rest } = p;
        positions.set(id, colour ? { ...rest, colour } : rest);
      }),
  });
}

/** A new Lua file under scripts/, listed in the game's `scripts:`. */
function addScriptFile(name: string): string | null {
  const path = `scripts/${slug(name, "script")}.lua`;
  if (project.files[path] !== undefined) {
    $("saved").textContent = `There's already a ${path}.`;
    return null;
  }
  change((m) => {
    project = {
      ...project,
      files: { ...project.files, [path]: `-- Functions for your scripts. Only functions go here: code\n-- outside them can't run.\n\nfunction greet(who)\n  say("Hello, " .. who .. ".")\nend\n` },
    };
    const list = m.list(["scripts"]);
    if (list.length) m.setList(["scripts"], [...list, path]);
    else m.addSection("scripts", [path], ["game", "settings"]);
  });
  return path;
}

/** Shows a file in the Files tab: game.yaml, or a script. */
function openFile(path: string) {
  currentFile = project.files[path] === undefined ? "game.yaml" : path;
  const pick = $<HTMLSelectElement>("file");
  pick.replaceChildren(
    ...Object.keys(project.files)
      .filter((f) => f === "game.yaml" || f.endsWith(".lua"))
      .sort((a, b) => (a === "game.yaml" ? -1 : b === "game.yaml" ? 1 : a.localeCompare(b)))
      .map((f) => Object.assign(document.createElement("option"), { value: f, textContent: f, selected: f === currentFile })),
  );
  $<HTMLTextAreaElement>("yaml").value = currentFile === "game.yaml" ? (model?.toString() ?? project.files["game.yaml"]) : project.files[currentFile];
  $("yaml").setAttribute("aria-label", currentFile);
  $("yaml-msg").textContent = "";
  $("file-help").textContent =
    currentFile === "game.yaml"
      ? "This is your game's file. Changes on the map show up here, and changes here show up on the map."
      : "A Lua file for your scripts' shared functions. Problems in it show in the Problems tab.";
  showTab("yaml");
}

function select(sel: Selection) {
  selection = sel;
  redraw();
  if (sel?.kind === "room") mapView.reveal(sel.id);
  if ((sel?.kind === "thing" || sel?.kind === "character") && model) {
    const room = model.roomOf(sel.id);
    if (room && room !== "player") mapView.reveal(room);
  }
}

/** A new room `dir` of `from`, joined both ways. */
function addBeside(from: string, dir: Direction, at?: Point): string {
  let id = "";
  change((m) => {
    id = m.addRoom("New room");
    const p = positions.get(from);
    positions.set(id, at ? { x: Math.round(at.x / 10) * 10, y: Math.round(at.y / 10) * 10 } : p ? besideRoom(p, dir, positions) : { x: 0, y: 0 });
    m.connect(from, dir, id);
  });
  selection = { kind: "room", id };
  redraw();
  focusName();
  return id;
}

function addAt(at: Point) {
  let id = "";
  change((m) => {
    id = m.addRoom("New room");
    positions.set(id, { x: Math.round(at.x / 10) * 10, y: Math.round(at.y / 10) * 10 });
  });
  selection = { kind: "room", id };
  redraw();
  focusName();
}

/** A new room's (or thing's) Name field, selected, ready to type over. */
function focusName() {
  const label = [...$("inspector").querySelectorAll("label")].find((l) => l.textContent === "Name");
  const input = (label?.htmlFor && document.getElementById(label.htmlFor)) as HTMLInputElement | null;
  input?.focus();
  input?.select();
}

// ------------------------------------------------- problems and the file

let engine: Promise<Engine> | null = null;
let checkTimer = 0;

function scheduleCheck() {
  testPlay.changed(project.files);
  clearTimeout(checkTimer);
  checkTimer = window.setTimeout(async () => {
    engine ??= loadEngine(new URL("./wasm/", location.href));
    const files: Files = { ...project.files };
    try {
      problems = (await engine).check(files).diagnostics;
    } catch (e) {
      problems = [{ severity: "error", path: "", message: `The checker couldn't run: ${e}` }];
    }
    showProblems();
    refreshPanel();
  }, 350);
}

/**
 * Shows new problems in the details panel, without taking the cursor from a
 * field being typed in: then it waits until the cursor leaves the panel.
 */
let panelStale = false;
function refreshPanel() {
  if ($("inspector").contains(document.activeElement)) {
    panelStale = true;
    drawMap();
  } else redraw();
}
$("inspector").addEventListener("focusout", (e) => {
  if (!panelStale || $("inspector").contains(e.relatedTarget as Node | null)) return;
  panelStale = false;
  // After the click that moved focus has done its work.
  setTimeout(() => {
    if (!$("inspector").contains(document.activeElement)) redraw();
    else panelStale = true;
  }, 0);
});

function showProblems() {
  const panel = $("panel-problems");
  const errors = problems.filter((p) => p.severity === "error").length;
  $("tab-problems").textContent = problems.length ? `Problems (${problems.length})` : "Problems";
  if (!problems.length) {
    panel.replaceChildren(el("p", { className: "ok-msg", textContent: "No problems: the game is ready to play." }));
    return;
  }
  const ul = el("ul", { className: "problems" });
  for (const d of problems) {
    const li = el("li", { className: d.severity, title: "Show it" });
    li.append(el("span", { className: "sev", textContent: d.severity === "error" ? "Error" : "Warning" }));
    if (d.path) li.append(el("code", { textContent: d.path }));
    li.append(d.message);
    li.addEventListener("click", () => goTo(d.path));
    ul.append(li);
  }
  panel.replaceChildren(
    el("p", { className: "dim", textContent: errors ? "The game can't be played until the errors are fixed. Click one to go to it." : "Warnings don't stop the game, but are worth a look." }),
    ul,
  );
}

/** Selects what a problem's path is about, or shows it in the file. */
function goTo(path: string) {
  const parts = path.split(/[.[\]]+/).filter(Boolean);
  if (parts[0] === "rooms" && parts[1] && model?.room(parts[1])) {
    if (parts[2] === "exits" && parts[3] && model.exit(parts[1], parts[3] as Direction)) select({ kind: "exit", room: parts[1], dir: parts[3] as Direction });
    else select({ kind: "room", id: parts[1] });
    return;
  }
  if (parts[0] === "objects" && parts[1] && model?.door(parts[1])) {
    for (const r of model.roomIds()) {
      const e = model.exits(r).find((x) => x.door === parts[1]);
      if (e) return select({ kind: "exit", room: r, dir: e.dir });
    }
  }
  if (parts[0] === "objects" && parts[1] && model?.thing(parts[1])) return select({ kind: "thing", id: parts[1] });
  if (parts[0] === "characters" && parts[1] && model?.character(parts[1])) return select({ kind: "character", id: parts[1] });
  if (parts[0] === "factions" && parts[1] && model?.faction(parts[1])) return select({ kind: "faction", id: parts[1] });
  for (const [kind, section] of [["ending", "endings"], ["timer", "timers"], ["choice", "choices"], ["verb", "verbs"]] as const) {
    if (parts[0] === section && parts[1] && model?.ids(section).includes(parts[1])) return select({ kind, id: parts[1] });
  }
  if (parts[0] === "on") return select(null);
  if (parts[0] === "scripts") {
    // "scripts[0]", or a load error naming the file.
    const files = model?.list(["scripts"]) ?? [];
    const named = files.find((f) => problems.some((d) => d.path === path && d.message.includes(f)));
    const at = parts[1] ? files[Number(parts[1])] : undefined;
    return openFile(named ?? at ?? files[0] ?? "game.yaml");
  }
  showInFile(parts[1] ? `${parts[1]}:` : parts[0] ?? "");
}

function showTab(which: "problems" | "yaml" | "play") {
  for (const t of ["problems", "yaml", "play"] as const) {
    $(`tab-${t}`).setAttribute("aria-selected", String(which === t));
    $(`panel-${t}`).hidden = which !== t;
  }
  testPlay.shown(which === "play");
}



function showInFile(find: string) {
  if (currentFile !== "game.yaml") openFile("game.yaml");
  showTab("yaml");
  const ta = $<HTMLTextAreaElement>("yaml");
  const at = ta.value.search(new RegExp(`^\\s*${find.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}`, "m"));
  if (at < 0) return;
  ta.focus();
  ta.setSelectionRange(at, ta.value.indexOf("\n", at));
  // Scroll the line into view: about 1.45em per line.
  const line = ta.value.slice(0, at).split("\n").length;
  ta.scrollTop = Math.max(0, (line - 3) * parseFloat(getComputedStyle(ta).lineHeight || "18"));
}

let typingYaml = false;
function syncYaml(text: string) {
  if (!typingYaml && currentFile === "game.yaml") $<HTMLTextAreaElement>("yaml").value = text;
}

let yamlTimer = 0;
$("yaml").addEventListener("input", () => {
  typingYaml = true;
  clearTimeout(yamlTimer);
  yamlTimer = window.setTimeout(() => {
    const text = $<HTMLTextAreaElement>("yaml").value;
    if (currentFile !== "game.yaml") {
      // A script file: kept as typed; the checker says if it's broken.
      const before = snapshot();
      project = { ...project, files: { ...project.files, [currentFile]: text } };
      if (!sameFiles(before.files, snapshot().files)) {
        undo.push(before);
        redo.length = 0;
        save();
        scheduleCheck();
      }
      typingYaml = false;
      return;
    }
    try {
      const next = new Model(text);
      const before = snapshot();
      undo.push(before);
      redo.length = 0;
      model = next;
      positions = layout(model, positions);
      project = { ...project, files: { ...project.files, "game.yaml": text } };
      $("yaml-msg").textContent = "";
      save();
      redraw();
      scheduleCheck();
    } catch (e) {
      $("yaml-msg").textContent = `Not applied yet: ${e instanceof Error ? e.message : e}`;
    }
    typingYaml = false;
  }, 600);
});
$("tab-problems").addEventListener("click", () => showTab("problems"));
$("tab-play").addEventListener("click", () => showTab("play"));
$("tab-yaml").addEventListener("click", () => openFile(currentFile));
$("file").addEventListener("change", () => openFile($<HTMLSelectElement>("file").value));

// ------------------------------------------------------------ toolbar

$("undo").addEventListener("click", doUndo);
$("redo").addEventListener("click", doRedo);
$("play").addEventListener("click", () => {
  save();
  window.open(`./play.html?local=${encodeURIComponent(project.key)}&fresh=1`, "_blank", "noopener");
});
$("download").addEventListener("click", () => {
  save();
  const id = (typeof model?.get(["game", "id"]) === "string" ? (model.get(["game", "id"]) as string) : "game").replace(/[^\w-]/g, "_");
  const zip = zipSync(Object.fromEntries(Object.entries(project.files).map(([p, t]) => [`${id}/${p}`, strToU8(t)])));
  const a = el("a", { href: URL.createObjectURL(new Blob([zip], { type: "application/zip" })), download: `${id}.zip` });
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 1000);
});
// The map's look: plain, or a blueprint. Remembered in this browser.
function showBlueprint(on: boolean) {
  document.querySelector(".canvas")?.classList.toggle("blueprint", on);
  $("blueprint").setAttribute("aria-pressed", String(on));
}
showBlueprint(blueprint.on());
$("blueprint").addEventListener("click", () => {
  const on = !blueprint.on();
  blueprint.set(on);
  showBlueprint(on);
});
$("add-room").addEventListener("click", () => {
  const r = $("map").getBoundingClientRect();
  addAt(mapView.toMap(r.left + r.width / 2, r.top + r.height / 2));
});
$("zoom-in").addEventListener("click", () => mapView.zoom(1.25));
$("zoom-out").addEventListener("click", () => mapView.zoom(0.8));
$("fit").addEventListener("click", () => mapView.fit());

document.addEventListener("keydown", (e) => {
  const typing = e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLSelectElement;
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "z" && !typing) {
    e.preventDefault();
    if (e.shiftKey) doRedo();
    else doUndo();
  } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "y" && !typing) {
    e.preventDefault();
    doRedo();
  } else if ((e.key === "Delete" || e.key === "Backspace") && !typing && model && selection) {
    e.preventDefault();
    if (selection.kind === "character") {
      const c = model.character(selection.id);
      if (c && confirm(`Delete ${c.name}? What they carry is left where they were.`)) {
        const id = selection.id;
        selection = c.in && model.room(c.in) ? { kind: "room", id: c.in } : null;
        change((m) => m.deleteCharacter(id));
      }
    } else if (selection.kind === "faction") {
      const id = selection.id;
      if (confirm(`Delete the side “${id}”? Its members stay, on no side.`)) {
        selection = null;
        change((m) => m.deleteFaction(id));
      }
    } else if (selection.kind === "ending" || selection.kind === "timer" || selection.kind === "choice" || selection.kind === "verb") {
      const { kind, id } = selection;
      const section = kind === "choice" ? "choices" : `${kind}s`;
      if (confirm(`Delete the ${kind === "choice" ? "conversation" : kind} “${id}”?`)) {
        selection = null;
        change((m) => m.deleteEntry(section, id));
      }
    } else if (selection.kind === "thing") {
      const t = model.thing(selection.id);
      if (t && confirm(`Delete ${t.name}?`)) {
        const id = selection.id;
        selection = t.in && model.room(t.in) ? { kind: "room", id: t.in } : null;
        change((m) => m.deleteThing(id));
      }
    } else if (selection.kind === "room") {
      const r = model.room(selection.id);
      if (r && confirm(`Delete ${r.name}, and the ways into it?`)) {
        const id = selection.id;
        selection = null;
        change((m) => {
          m.deleteRoom(id);
          positions.delete(id);
        });
      }
    } else {
      const { room, dir } = selection;
      const ex = model.exit(room, dir);
      const back = ex?.to ? model.wayBack(room, ex.to, dir) : undefined;
      selection = { kind: "room", id: room };
      change((m) => {
        m.removeExit(room, dir);
        if (back && ex?.to) m.removeExit(ex.to, back.dir);
      });
    }
  } else if (e.key === "Escape" && !typing && selection) {
    select(null);
  }
});

mapView = new MapView(document.getElementById("map") as unknown as SVGSVGElement, {
  select,
  moveRoom: (id, to) => change(() => positions.set(id, { ...positions.get(id), ...to }), { coalesce: `move:${id}` }),
  resizeRoom: (id, size) =>
    change(() => {
      const p = positions.get(id);
      if (!p) return;
      positions.set(id, size ? { ...p, ...size } : { x: p.x, y: p.y, ...(p.colour ? { colour: p.colour } : {}) });
    }),
  connect: (from, dir, to) => {
    change((m) => m.connect(from, dir, to));
    selection = { kind: "exit", room: from, dir };
    redraw();
  },
  addRoomBeside: (from, dir, at) => void addBeside(from, dir, at),
  addRoomAt: addAt,
  dropThing: (id, room) => {
    if (model?.character(id)) {
      change((m) => m.moveCharacter(id, room));
      selection = { kind: "character", id };
    } else {
      change((m) => m.moveThing(id, room));
      selection = { kind: "thing", id };
    }
    redraw();
  },
  moveCharacter: (id, room) => {
    change((m) => m.moveCharacter(id, room));
    selection = { kind: "character", id };
    redraw();
  },
});

// ------------------------------------------------------- start screen

async function startScreen() {
  $("start").hidden = false;
  $("workspace").hidden = true;
  const mine = $("mine");
  const games = localGames.list();
  mine.replaceChildren(
    ...(games.length
      ? games.map((g) => el("li", {}, el("span", { className: "name", textContent: g.title }), button("Edit", () => open(g), "primary")))
      : [el("li", { className: "dim", textContent: "None yet. Games you make or load are kept here, in this browser." })]),
  );
  $("new-game").addEventListener("click", () => {
    const g: LocalGame = { key: newKey(), title: "My Game", files: { "game.yaml": NEW_GAME }, added: Date.now() };
    if (!localGames.put(g)) return void ($("open-msg").textContent = "There's no room left in this browser's storage.");
    open(g);
  });
  try {
    const list: { id: string; title: string; about: string | null }[] = await (await fetch("./games/index.json")).json();
    $("examples").replaceChildren(
      ...list.map((x) =>
        el(
          "li",
          {},
          el("span", { className: "name", textContent: x.title }),
          el("span", { className: "dim", textContent: x.about ?? "" }),
          button("Edit a copy", async () => {
            const files: Files = await (await fetch(`./games/${x.id}.json`)).json();
            const g: LocalGame = { key: newKey(), title: `${x.title} (copy)`, files, added: Date.now() };
            if (localGames.put(g)) open(g);
          }),
        ),
      ),
    );
  } catch {
    $("examples").replaceChildren(el("li", { className: "dim", textContent: "The examples couldn't be loaded." }));
  }
  const take = async (list: FileList) => {
    const files = await fromFileList(list).catch((e: unknown) => `That couldn't be read: ${e}`);
    if (typeof files === "string") return void ($("open-msg").textContent = files);
    let title = "Untitled game";
    try {
      title = new Model(files["game.yaml"]).title();
    } catch {
      /* opens on the file tab */
    }
    const g: LocalGame = { key: newKey(), title, files, added: Date.now() };
    if (!localGames.put(g)) return void ($("open-msg").textContent = "There's no room left in this browser's storage.");
    open(g);
  };
  const folder = $<HTMLInputElement>("folder");
  const files = $<HTMLInputElement>("files");
  $("pick-folder").addEventListener("click", () => folder.click());
  $("pick-files").addEventListener("click", () => files.click());
  for (const input of [folder, files]) input.addEventListener("change", () => input.files?.length && take(input.files));
}

// The standard verbs, for the script picker.
fetch("./verbs.json")
  .then((r) => r.json())
  .then((v) => {
    verbs = v;
    if (model) redraw();
  })
  .catch(() => {
    verbs = ["take", "drop", "open", "close", "examine", "search", "push", "pull", "eat", "wear", "talk", "give", "attack", "unlock", "put_in"].map((id) => ({ id, example: id }));
  });

const key = new URLSearchParams(location.search).get("project") ?? new URLSearchParams(location.search).get("local");
const existing = key ? localGames.get(key) : null;
if (existing) open(existing);
else startScreen();
