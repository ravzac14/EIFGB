// The editor page: choosing a project, then the map, the details panel and
// the drawer (problems, and the game file). Projects are kept in the
// browser with the games you've loaded, so they're on the home page too.

import { dataBytes, fileBytes, isPicture, pictureSrc, shrink } from "../pictures.ts";
import { zipSync, strToU8 } from "fflate";
import { loadEngine, type Diagnostic, type Engine, type Files, type Puzzles, type Walkthrough } from "../engine.ts";
import { fromFileList } from "../files.ts";
import { localGames, mapTheme, storageUse, type LocalGame, type MapTheme } from "../store.ts";
import { settingsDialog } from "../settings-ui.ts";
import { feedbackButton, feedbackContext } from "../feedback.ts";
import { Model, NEW_GAME, slug, type Direction } from "./model.ts";
import { MAP_FILE, besideRoom, layout, readMap, readTalk, writeMap, type Point, type Positions } from "./layout.ts";
import { TalkView, talkLayout, type TalkPositions } from "./talk-graph.ts";
import { MapView, type Selection } from "./map.ts";
import { renderInspector, type InspectorContext } from "./inspector.ts";
import { openAddComponent } from "./add-component.ts";
import { TestPlay, WALKTHROUGH } from "./testplay.ts";

/** walkthrough.txt, or one for an ending: walkthrough-feast.txt. */
const isWalkthrough = (f: string) => f === WALKTHROUGH || /^walkthrough-[^/]+\.txt$/.test(f);
import { describeChange, when } from "./history.ts";
import { findInGame, MAX_FOUND } from "./find.ts";
import { mapProblems, tidy, wayKey, type MapProblem, type Way } from "./tidy.ts";
import { draftHints } from "./hint-drafts.ts";
import { typos } from "./spelling.ts";
import { copyText, forChat, fromChat, instructions } from "./ai.ts";
import { timeline, timeText } from "./timeline.ts";

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
/** Where each conversation's parts sit in its graph (only those placed by hand). */
let talkPos: TalkPositions = new Map();
/** The conversation part last clicked in the graph. */
let talkPicked: string | null = null;
let selection: Selection = null;
let problems: Diagnostic[] = [];
/** How each walkthrough played, at the last check. */
let walkthroughs: Walkthrough[] = [];
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
  saveWalkthrough: (text, file = WALKTHROUGH) =>
    change(() => {
      project = { ...project, files: { ...project.files, [file]: text } };
    }),
  nameOf: (id) => model?.room(id)?.name ?? model?.thing(id)?.name ?? model?.character(id)?.name ?? id.replace(/_/g, " "),
  show: (what) => {
    if ("choice" in what) return select({ kind: "choice", id: what.choice });
    if (model?.room(what.id)) return select({ kind: "room", id: what.id });
    if (model?.character(what.id)) return select({ kind: "character", id: what.id });
    goTo(`objects.${what.id}`);
  },
  thingsIn: (room) =>
    model
      ? [
          ...model.things().filter((t) => model!.roomOf(t.id) === room).map((t) => ({ id: t.id, name: t.name, section: "objects" as const })),
          ...model.doors().filter((d) => model!.exits(room).some((e) => e.door === d.id)).map((d) => ({ id: d.id, name: d.name, section: "objects" as const })),
          ...model.characterIds().filter((c) => model!.character(c)?.in === room).map((c) => ({ id: c, name: model!.character(c)!.name, section: "characters" as const })),
        ]
      : [],
  verbWords: () => {
    const std = verbs.map((v) => v.example.split(" ")[0]);
    const own = model?.ids("verbs") ?? [];
    return [...new Set([...std, ...own, "look", "inventory", "wait"])].filter((w) => /^[a-z]+$/.test(w)).sort();
  },
  addSynonym: (section, id, word) =>
    change((m) => m.setList([section, id, "synonyms"], [...m.list([section, id, "synonyms"]), word])),
  addAlias: (word, means) => change((m) => m.setAlias(word, means)),
  addVerb: (word) => {
    let id = "";
    change((m) => (id = m.addVerb(word)));
    select({ kind: "verb", id });
  },
  addScenery: (room, word) => {
    let id = "";
    change((m) => (id = m.addThing("scenery", room, word)));
    select({ kind: "thing", id });
  },
});

/**
 * Every file of the project at one moment: what undo goes back to. On the
 * undo and redo lists, `label` says what the change from (or to) it was,
 * and `at` when it was made.
 */
interface Snapshot {
  files: Files;
  label?: string;
  at?: number;
}
const undo: Snapshot[] = [];
const redo: Snapshot[] = [];
let lastCoalesce: { key: string; at: number } | null = null;

const snapshot = (): Snapshot => ({
  files: { ...project.files, "game.yaml": model ? model.toString() : project.files["game.yaml"], ...(model ? { [MAP_FILE]: mapText() } : {}) },
});
/** Puts `before` on the undo list, labelled with the change since (or as given). */
function pushUndo(before: Snapshot, after: Snapshot, label?: string) {
  undo.push({ files: before.files, label: label ?? describeChange(before.files, after.files), at: Date.now() });
  redo.length = 0;
}
const sameFiles = (a: Files, b: Files) => {
  const keys = new Set([...Object.keys(a), ...Object.keys(b)]);
  return [...keys].every((k) => a[k] === b[k]);
};
/** The standard verbs, for the script picker (loaded at start). */
let verbs: { id: string; example: string }[] = [];
/** The file open in the Files tab. */
let currentFile = "game.yaml";
const mapText = () =>
  model
    ? writeMap(positions, model.roomIds(), talkPos, Object.fromEntries(model.choiceIds().map((c) => [c, model!.choice(c)?.nodes.map((n) => n.id) ?? []])))
    : (project.files[MAP_FILE] ?? "");

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
    talkPos = readTalk(map);
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
  for (const [kind, section] of [["ending", "endings"], ["timer", "timers"], ["hint", "hints"], ["goal", "goals"], ["choice", "choices"], ["verb", "verbs"]] as const) {
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
function change(fn: (m: Model) => void, opts: { coalesce?: string; redraw?: boolean; label?: string } = {}) {
  if (!model) return;
  const before = snapshot();
  model.lastRename = null;
  fn(model);
  // A rename reaches the script files, and a room keeps its place on the map.
  const renamed = model.lastRename as Model["lastRename"];
  if (renamed) {
    const files = { ...project.files };
    for (const [p, t] of Object.entries(files)) if (p.endsWith(".lua")) files[p] = renamed.code(t);
    project = { ...project, files };
    const at = renamed.kind === "entity" ? positions.get(renamed.old) : undefined;
    if (at) {
      positions.set(renamed.next, at);
      positions.delete(renamed.old);
    }
  }
  const after = snapshot();
  if (!sameFiles(before.files, after.files)) {
    const now = Date.now();
    const merge = opts.coalesce && lastCoalesce?.key === opts.coalesce && now - lastCoalesce.at < 2000 && undo.length;
    if (merge) {
      // Still typing in the same field: the same step, now saying all of it.
      const top = undo[undo.length - 1];
      top.label = describeChange(top.files, after.files);
      top.at = now;
      redo.length = 0;
    } else pushUndo(before, after, opts.label);
    lastCoalesce = opts.coalesce ? { key: opts.coalesce, at: now } : null;
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
  redo.push({ ...snapshot(), label: s.label, at: s.at });
  restore(s);
}
function doRedo() {
  const s = redo.pop();
  if (!s) return;
  undo.push({ ...snapshot(), label: s.label, at: s.at });
  restore(s);
}

/** Goes back (or forward) `n` changes at once, from the history list. */
function stepMany(n: number, from: Snapshot[], to: Snapshot[]) {
  let now = snapshot();
  for (let i = 0; i < n && from.length; i++) {
    const s = from.pop()!;
    to.push({ files: now.files, label: s.label, at: s.at });
    now = s;
  }
  restore(now);
}
const undoMany = (n: number) => stepMany(n, undo, redo);
const redoMany = (n: number) => stepMany(n, redo, undo);

/** How many changes the history list shows. */
const HISTORY = 30;

/** The history list: recent changes, newest first, to step back to. */
function showHistory() {
  const dialog = $<HTMLDialogElement>("history-dialog");
  const list = el("ol", { className: "history" });
  // Changes undone, which can be redone: newest-undone last, so they read in order.
  const undone = redo.slice(-HISTORY);
  for (let i = 0; i < undone.length; i++) {
    const s = undone[i];
    const steps = undone.length - i;
    const li = el("li", { className: "undone" }, el("span", { className: "what", textContent: s.label ?? "A change" }), el("span", { className: "dim", textContent: " undone" }));
    li.append(button("Redo to here", () => (redoMany(steps), dialog.close())));
    list.append(li);
  }
  list.append(el("li", { className: "now" }, el("strong", { textContent: "Now" })));
  const done = undo.slice(-HISTORY).reverse();
  done.forEach((s, i) => {
    const li = el("li", {}, el("span", { className: "what", textContent: s.label ?? "A change" }), el("span", { className: "dim", textContent: ` ${s.at ? when(s.at) : ""}` }));
    li.append(button(i === 0 ? "Undo this" : `Undo back to here (${i + 1} changes)`, () => (undoMany(i + 1), dialog.close())));
    list.append(li);
  });
  dialog.replaceChildren(
    button("Close", () => dialog.close(), "close"),
    el("h2", { id: "history-title", textContent: "History" }),
    el("p", { className: "dim", textContent: undo.length || redo.length ? "Your changes since you opened the game, newest first. Going back can be redone, until you change something else." : "No changes yet since you opened the game." }),
    list,
    undo.length > HISTORY ? el("p", { className: "dim", textContent: `And ${undo.length - HISTORY} earlier changes: Undo reaches those too.` }) : "",
  );
  dialog.showModal();
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

/** A conversation, when one is selected: its graph instead of the map. */
function drawTalk(): boolean {
  const c = selection?.kind === "choice" && model ? model.choice(selection.id) : null;
  $("map").toggleAttribute("hidden", !!c);
  $("talk").toggleAttribute("hidden", !c);
  $("talk-bar").hidden = !c;
  if (!c) return false;
  if (talkPicked && !c.nodes.some((n) => n.id === talkPicked)) talkPicked = null;
  talkView.render(c, talkLayout(c, talkPos.get(c.id)), talkPicked);
  $("talk-title").textContent = `The conversation “${c.id.replace(/_/g, " ")}”`;
  return true;
}

function drawMap() {
  const talking = drawTalk();
  const hard = model && !talking ? mapProblems(model, positions) : [];
  const flagged = new Set(hard.flatMap((p) => (p.kind === "cross" ? [wayKey(p.a), wayKey(p.b)] : [wayKey(p.a)])));
  if (model) mapView.render(model, positions, selection, badRooms(), flagged);
  showMapCheck(hard);
  $<HTMLButtonElement>("undo").disabled = !undo.length;
  $<HTMLButtonElement>("redo").disabled = !redo.length;
  $<HTMLButtonElement>("history").disabled = !undo.length && !redo.length;
  $("undo").title = undo.length ? `Undo: ${undo[undo.length - 1].label ?? "the last change"} (Ctrl+Z)` : "Undo (Ctrl+Z)";
  $("redo").title = redo.length ? `Redo: ${redo[redo.length - 1].label ?? "the change"} (Ctrl+Shift+Z)` : "Redo (Ctrl+Shift+Z)";
  $("title").textContent = model?.title() ?? project.title;
  document.title = `${model?.title() ?? project.title} · Editor`;
  $("map-hint").textContent = talking
    ? "Drag a part to move it. Drag from a reply's dot onto a part to make it lead there, or into empty space for a new part. Esc goes back to the map."
    : selection?.kind === "room"
      ? "Click a + to add a room that way, or drag it onto another room to connect them."
      : model && model.roomIds().length < 2
        ? "Click a room, then one of the + around it to add a room next door."
        : "";
}

/** Under the map's tools: ways that are hard to read where they're drawn. */
function showMapCheck(hard: MapProblem[]) {
  const box = $("map-check");
  box.hidden = !hard.length;
  if (!hard.length) return box.replaceChildren();
  const name = (id: string) => model?.room(id)?.name ?? id;
  const way = (w: Way) => {
    const b = button(`${name(w.room)}–${name(w.to)}`, () => select({ kind: "exit", room: w.room, dir: w.dir }), "link");
    b.title = "Select it";
    return b;
  };
  const items = hard.slice(0, 5).map((p) =>
    el(
      "li",
      {},
      ...(p.kind === "cross"
        ? [way(p.a), " crosses ", way(p.b)]
        : p.kind === "through"
          ? [way(p.a), ` runs through ${name(p.room)}`]
          : [way(p.a), ` goes ${p.a.dir}, but is drawn the other way`]),
    ),
  );
  const details = el("details", {}, el("summary", { textContent: `${hard.length} way${hard.length === 1 ? " is" : "s are"} hard to read on the map` }), el("ul", {}, ...items));
  if (hard.length > 5) details.append(el("p", { className: "dim", textContent: `And ${hard.length - 5} more.` }));
  details.append(el("p", { className: "dim", textContent: "Drag rooms to untangle them, or Tidy up to lay the map out again by its directions. Only the map changes, never the game." }));
  box.replaceChildren(details);
}

/** The Timers tab: when each timer can go off, and what it does. */
function showTimers() {
  const tab = $("tab-timers");
  const panel = $("panel-timers");
  const ids = model?.ids("timers") ?? [];
  tab.hidden = !ids.length;
  if (!ids.length) {
    if (!panel.hidden) showTab("problems");
    return panel.replaceChildren();
  }
  const t = timeline(model!);
  const pct = (n: number) => `${Math.min(100, (n / t.span) * 100)}%`;
  // The scale: actions, and with a clock the time they'd reach.
  const step = t.span <= 40 ? 5 : t.span <= 100 ? 10 : t.span <= 400 ? 50 : 100;
  const scale = el("div", { className: "tl-scale" });
  for (let n = 0; n <= t.span; n += step) {
    const tick = el("span", { className: "tl-tick", textContent: String(n) });
    tick.style.left = pct(n);
    if (t.clock) tick.title = timeText(t.clock.start + n * t.clock.perTurn);
    scale.append(tick);
    if (t.clock && n % (step * 2) === 0) {
      const clock = el("span", { className: "tl-tick tl-time", textContent: timeText(t.clock.start + n * t.clock.perTurn) });
      clock.style.left = pct(n);
      scale.append(clock);
    }
  }
  const rows = t.timers.map((p) => {
    const track = el("div", { className: `tl-track${p.fromStart ? "" : " later"}` });
    for (const m of p.marks) {
      if (m.from > t.span) continue;
      const mark = el("span", { className: m.to > m.from ? "tl-span" : "tl-mark" });
      mark.style.left = pct(m.from);
      if (m.to > m.from) mark.style.width = `calc(${pct(m.to)} - ${pct(m.from)})`;
      const at = t.clock && p.fromStart ? `, about ${timeText(t.clock.start + m.from * t.clock.perTurn)}` : "";
      mark.title = m.to > m.from ? `Between actions ${m.from} and ${m.to}${at}` : `After action ${m.from}${at}`;
      track.append(mark);
    }
    const name = button(p.id, () => select({ kind: "timer", id: p.id }), "link");
    name.title = "Select it";
    return el(
      "li",
      { className: "tl-row" },
      el("div", { className: "tl-label" }, name, el("span", { className: "dim", textContent: ` ${p.schedule}, ${p.fromStart ? "from the start" : "counting from " + p.starts.replace(/^when /, "when ")}` })),
      track,
      el("p", { className: "tl-does" }, `It ${p.does.join(", ")}.`, p.unless.length ? ` ${p.unless.map((u) => u.charAt(0).toUpperCase() + u.slice(1)).join(". ")}.` : "", p.stops ? ` Stopped by: ${p.stops}.` : ""),
    );
  });
  panel.replaceChildren(
    el("p", { className: "dim", textContent: `When each timer can go off, counted in actions (commands that take a turn)${t.clock ? `, and the time of day that would be (the clock starts at ${timeText(t.clock.start)}, ${t.clock.perTurn} minute${t.clock.perTurn === 1 ? "" : "s"} an action)` : ""}. A shaded span is a random wait; a dashed track counts from when something starts the timer, not from the start.` }),
    el("div", { className: "tl" }, scale, el("ol", { className: "tl-rows" }, ...rows)),
  );
}

function redraw() {
  drawMap();
  testPlay.update();
  showTimers();
  if (!model) {
    $("inspector").replaceChildren(el("p", { className: "dim", textContent: "Fix the mistake in the game file to see the map." }));
    return;
  }
  renderInspector($("inspector"), inspectorContext(model));
}

/** What the details panel (and Add component) can see and do. */
function inspectorContext(model: Model): InspectorContext {
  return {
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
    addPicture,
    pictureUrl: (file) => pictureSrc(file, project.files),
    pictures: () => {
      const users = new Map<string, { owner: string[]; name: string }[]>();
      const kinds = [["rooms", "Room"], ["objects", "Thing"], ["characters", "Character"]] as const;
      for (const [section, kind] of kinds) {
        for (const id of model.ids(section)) {
          const file = model.picture([section, id])?.file;
          if (!file) continue;
          const name = (section === "rooms" ? model.room(id)?.name : section === "objects" ? model.thing(id)?.name : model.character(id)?.name) ?? id;
          users.set(file, [...(users.get(file) ?? []), { owner: [section, id], name: `${kind} ${name}` }]);
        }
      }
      return Object.keys(project.files)
        .filter((f) => isPicture(f, project.files[f]))
        .sort()
        .map((file) => ({ file, bytes: fileBytes(project.files[file]), usedBy: users.get(file) ?? [] }));
    },
    storage: storageUse,
    hasWalkthrough: () => project.files[WALKTHROUGH] !== undefined,
    dropPicture: (file) => {
      if (project.files[file] === undefined || model?.toString().includes(file)) return;
      const { [file]: _gone, ...rest } = project.files;
      change(() => (project = { ...project, files: rest }));
    },
    roomColour: (id) => positions.get(id)?.colour,
    setRoomColour: (id, colour) =>
      change(() => {
        const p = positions.get(id);
        if (!p) return;
        const { colour: _old, ...rest } = p;
        positions.set(id, colour ? { ...rest, colour } : rest);
      }),
  };
}

/** A picture the author chose, made small, under pictures/. Returns its path. */
async function addPicture(file: File, base: string): Promise<string | null> {
  let shrunk;
  try {
    shrunk = await shrink(file);
  } catch {
    $("saved").textContent = "That picture couldn't be opened.";
    return null;
  }
  const ext = shrunk.name.split(".").pop()!.toLowerCase();
  let path = `pictures/${slug(base, "picture")}.${ext}`;
  for (let i = 2; project.files[path] !== undefined && project.files[path] !== shrunk.url; i++) path = `pictures/${slug(base, "picture")}_${i}.${ext}`;
  project = { ...project, files: { ...project.files, [path]: shrunk.url } };
  return path;
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
      .filter((f) => f === "game.yaml" || isWalkthrough(f) || f.endsWith(".lua"))
      .sort((a, b) => (a === "game.yaml" ? -1 : b === "game.yaml" ? 1 : a.localeCompare(b)))
      .map((f) => Object.assign(document.createElement("option"), { value: f, textContent: f, selected: f === currentFile })),
  );
  $<HTMLTextAreaElement>("yaml").value = currentFile === "game.yaml" ? (model?.toString() ?? project.files["game.yaml"]) : project.files[currentFile];
  $("yaml").setAttribute("aria-label", currentFile);
  $("yaml-msg").textContent = "";
  $("file-help").textContent =
    currentFile === "game.yaml"
      ? "This is your game's file. Changes on the map show up here, and changes here show up on the map."
      : isWalkthrough(currentFile)
        ? `Commands that ${currentFile === WALKTHROUGH ? "win the game" : `reach the ${currentFile.slice(12, -4)} ending`}, one a line (# starts a comment). A line can say what the game should answer: take key => Taken. After every change, the editor plays them and says under Problems if they no longer do. Test play can record a new one.`
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
      const checked = (await engine).check(files);
      problems = checked.diagnostics;
      puzzles = checked.puzzles ?? null;
      testPlay.walkthrough(files[WALKTHROUGH] === undefined ? "none" : (checked.walkthrough ?? "unchecked"), files[WALKTHROUGH]);
      walkthroughs = checked.walkthroughs ?? [];
      testPlay.walkthroughs(walkthroughs);
    } catch (e) {
      problems = [{ severity: "error", path: "", message: `The checker couldn't run: ${e}` }];
    }
    showProblems();
    showPuzzles();
    refreshPanel();
  }, 350);
}

let puzzles: Puzzles | null = null;

/**
 * The Puzzles tab: what can't happen, where a player could get stuck, then
 * what the player does for each ending (only the steps it needs), and side
 * puzzles nothing needs.
 */
function showPuzzles() {
  const panel = $("panel-puzzles");
  const bad = puzzles?.problems ?? [];
  const worries = bad.length + (puzzles?.stuck?.length ?? 0);
  $("tab-puzzles").textContent = worries ? `Puzzles (${worries})` : "Puzzles";
  if (!puzzles) {
    panel.replaceChildren(el("p", { className: "dim", textContent: "The puzzle chain shows once the game's errors are fixed." }));
    return;
  }
  const go = (what: string, id: string) => {
    if (what === "ending") return goTo(`endings.${id}`);
    if (what === "choice" && model?.ids("choices").includes(id)) return select({ kind: "choice", id });
    if (model?.room(id)) return select({ kind: "room", id });
    if (model?.character(id)) return select({ kind: "character", id });
    goTo(`objects.${id}`);
  };
  const out: HTMLElement[] = [];
  if (bad.length) {
    out.push(el("h3", { textContent: "Can't happen" }));
    const ul = el("ul", { className: "problems" });
    for (const p of bad) {
      const li = el("li", { className: "warning", title: "Show it" });
      li.append(el("span", { className: "sev", textContent: p.what === "ending" ? "Ending" : p.what === "room" ? "Room" : "Thing" }), p.message);
      li.addEventListener("click", () => go(p.what, p.id));
      ul.append(li);
    }
    out.push(ul);
  }
  // Where a player could get stuck on the way to a win.
  const stuck = puzzles.stuck ?? [];
  if (stuck.length) {
    out.push(el("h3", { textContent: "Where a player could get stuck" }));
    const ul = el("ul", { className: "problems" });
    for (const p of stuck) {
      const li = el("li", { className: "warning", title: "Show it" });
      li.append(el("span", { className: "sev", textContent: "Stuck" }), p.message);
      li.addEventListener("click", () => go(p.what, p.id));
      ul.append(li);
    }
    out.push(ul);
  }
  const steps = puzzles.steps;
  // One step as a line: what to do, what it waits on, and other ways to do it.
  const stepLine = (i: number, numbered: Map<number, number>) => {
    const s = steps[i];
    const li = el("li", { className: `step ${s.kind}`, title: "Show it" }, el("span", { className: "what", textContent: s.text }));
    const after = (s.needs ?? []).filter((n) => numbered.has(n)).map((n) => numbered.get(n)!);
    if (after.length) li.append(el("span", { className: "after dim", textContent: ` after ${after.join(" and ")}` }));
    for (const o of s.or ?? []) li.append(el("span", { className: "or dim", textContent: `or: ${o}` }));
    if (s.either?.length) li.append(el("span", { className: "either", textContent: `Only one of these: it rules out ${s.either.map((x) => x.replace(/\.$/, "")).join(" and ")}.` }));
    if (s.script && s.kind !== "choose") {
      const open = button(`solved in a script (${s.script})`, () => openScript(s.script!), "link");
      open.title = "The chain can't see inside scripts: check this one does what the step says";
      open.addEventListener("click", (e) => e.stopPropagation());
      li.append(el("span", { className: "script" }, open));
    }
    li.addEventListener("click", () => go(s.kind === "ending" ? "ending" : s.kind === "choose" ? "choice" : "", s.id));
    return li;
  };
  // A chain of steps, from the start, each numbered for "after 2".
  const chain = (ids: number[]) => {
    const ol = el("ol", { className: "chain" });
    const numbered = new Map<number, number>();
    for (const i of [0, ...ids.filter((i) => i !== 0)]) {
      numbered.set(i, numbered.size + 1);
      ol.append(stepLine(i, numbered));
    }
    return ol;
  };
  const endings = puzzles.endings.filter((e) => e.reachable && e.needs);
  out.push(
    el("h3", { textContent: "What the player does" }),
    el("p", { className: "dim", textContent: "For each ending, what has to be found, unlocked and reached, each after what it needs. Steps that can be done more than one way say so. Conditions are assumed to come true." }),
  );
  if (endings.length) {
    for (const e of endings) {
      const at = steps.findIndex((s) => s.kind === "ending" && s.id === e.key);
      out.push(el("h4", { textContent: `The ${e.key} ending (${e.kind === "win" ? "a win" : e.kind === "loss" ? "a loss" : "neither"})${e.needs!.length ? `: ${e.needs!.length} step${e.needs!.length === 1 ? "" : "s"}` : ""}` }));
      if (e.only_if?.length) out.push(el("p", { className: "only-if", textContent: `${e.only_if.join(", and ")}.`.replace(/^o/, "O") }));
      out.push(chain([...e.needs!, ...(at >= 0 ? [at] : [])]));
    }
    const used = new Set(endings.flatMap((e) => e.needs!));
    const side = steps.map((s, i) => i).filter((i) => i > 0 && steps[i].kind !== "ending" && !used.has(i));
    if (side.length) {
      out.push(el("h4", { textContent: "Side puzzles" }), el("p", { className: "dim", textContent: "Things the player can do that no ending needs." }));
      const ul = el("ul", { className: "chain side" });
      for (const i of side) ul.append(stepLine(i, new Map()));
      out.push(ul);
    }
  } else {
    out.push(chain(steps.map((_, i) => i)));
  }
  // What it takes on trust: said plainly, so the author can check those parts.
  const assumes = puzzles.assumes ?? [];
  if (assumes.length) {
    const ul = el("ul", { className: "problems assumes" });
    for (const a of assumes) {
      const li = el("li", { title: "Show it" }, a.message);
      li.addEventListener("click", () => go(a.what, a.id));
      ul.append(li);
    }
    out.push(
      el(
        "details",
        { className: "assumes-box" },
        el("summary", { textContent: `What the chain takes on trust (${assumes.length})` }),
        el("p", { className: "dim", textContent: "The chain reads the game; it doesn't play it. It can't see inside scripts, and it assumes conditions it can't read come true. These are the steps that rest on that: worth checking, or covering with the walkthrough in Test play." }),
        ul,
      ),
    );
  }
  // Hints for HINT, drafted from the steps: only the ones not drafted already.
  const have = new Set((model?.ids("hints") ?? []).map((h) => String(model!.get(["hints", h, "when"]) ?? "")));
  const drafts = model ? draftHints(model, puzzles.steps, puzzles.endings).filter((d) => !have.has(d.when)) : [];
  if (drafts.length) {
    const draft = button(`Draft ${drafts.length === 1 ? "a hint" : `${drafts.length} hints`} from these steps`, () => {
      let first = "";
      change(
        (m) => {
          for (const d of drafts) {
            const id = m.addHint(d.name);
            first ||= id;
            // `when` first, as the templates write it: when, then what to say.
            m.set(["hints", id, "steps"], undefined);
            m.set(["hints", id, "when"], d.when);
            m.setHintSteps(id, d.steps);
          }
        },
        { label: `Drafted ${drafts.length === 1 ? "a hint" : `${drafts.length} hints`} from the puzzle chain` },
      );
      if (first) select({ kind: "hint", id: first });
    });
    out.push(
      el(
        "div",
        { className: "draft-hints" },
        draft,
        el("p", { className: "dim", textContent: "For HINT: a nudge first and the answer last, for each step the game can tell is done (something found, a door unlocked). They're drafts: reword them in your own voice." }),
      ),
    );
  }
  if (puzzles.endings.length) {
    out.push(el("h3", { textContent: "Endings" }));
    const ul = el("ul", { className: "endings-list" });
    for (const e of puzzles.endings) {
      const li = el("li", { className: e.reachable ? "ok" : "bad", title: "Show it" });
      li.append(el("strong", { textContent: `${e.reachable ? "✓" : "✗"} ${e.key}` }), ` (${e.kind === "win" ? "a win" : e.kind === "loss" ? "a loss" : "neither"})`, e.from.length ? `: ${e.from.join(", ")}` : ": nothing leads here");
      li.addEventListener("click", () => go("ending", e.key));
      ul.append(li);
    }
    out.push(ul);
  }
  panel.replaceChildren(...out);
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

/** Possible typos, under the problems: not counted as problems, only worth a look. */
function typoList(): HTMLElement | "" {
  const found = typos(model?.toString() ?? project.files["game.yaml"] ?? "");
  if (!found.length) return "";
  const ul = el("ul", { className: "problems typos" });
  for (const t of found) {
    const li = el("li", { title: "Show it" }, el("span", { className: "sev", textContent: "Typo?" }), `“${t.word}”, perhaps “${t.like}”`, el("code", { textContent: ` ${t.path}` }));
    li.addEventListener("click", () => goTo(t.path));
    ul.append(li);
  }
  return el(
    "details",
    { className: "typos-box" },
    el("summary", { textContent: `Possible typos (${found.length})` }),
    el("p", { className: "dim", textContent: "Words used once that are one letter off a word the game uses elsewhere, or one of its names. Only a guess: the game's own words are always allowed. Text boxes are spell checked by the browser too." }),
    ul,
  );
}

function showProblems() {
  const panel = $("panel-problems");
  const errors = problems.filter((p) => p.severity === "error").length;
  $("tab-problems").textContent = problems.length ? `Problems (${problems.length})` : "Problems";
  // For a game written with an AI assistant: the problems, as a message to send it.
  const forAi = button("Copy for your AI", async () => {
    const ok = await copyText(forChat(problems, walkthroughs));
    forAi.textContent = ok ? "Copied: paste it into your chat" : "Couldn't copy: select the problems and copy them";
    setTimeout(() => (forAi.textContent = "Copy for your AI"), 2500);
  });
  forAi.title = "The problems and how the walkthrough went, as a message for the AI assistant writing your game";
  const aiRow = el("div", { className: "problems-bar" }, forAi);
  if (!problems.length) {
    panel.replaceChildren(el("p", { className: "ok-msg", textContent: "No problems: the game is ready to play." }), typoList());
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
    aiRow,
    el("p", { className: "dim", textContent: errors ? "The game can't be played until the errors are fixed. Click one to go to it." : "Warnings don't stop the game, but are worth a look." }),
    ul,
    typoList(),
  );
}

/** Selects what a problem's path is about, or shows it in the file. */
function goTo(path: string) {
  // The walkthrough: test play, where it can be played again and re-recorded.
  if (isWalkthrough(path)) return showTab("play");
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
  for (const [kind, section] of [["ending", "endings"], ["timer", "timers"], ["hint", "hints"], ["goal", "goals"], ["choice", "choices"], ["verb", "verbs"]] as const) {
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

type Tab = "problems" | "puzzles" | "timers" | "find" | "yaml" | "play";
const TABS: Tab[] = ["problems", "puzzles", "timers", "find", "yaml", "play"];
function showTab(which: Tab) {
  for (const t of TABS) {
    $(`tab-${t}`).setAttribute("aria-selected", String(which === t));
    $(`panel-${t}`).hidden = which !== t;
  }
  testPlay.shown(which === "play");
}



// ------------------------------------------------------------- find

/** Searches the whole game: the Find tab. */
function runFind() {
  const q = $<HTMLInputElement>("find-text").value;
  const out = $("find-results");
  const found = q.trim() ? findInGame(snapshot().files, q) : [];
  $("find-count").textContent = !q.trim() ? "" : found.length ? `${found.length >= MAX_FOUND ? `The first ${MAX_FOUND}` : found.length} place${found.length === 1 ? "" : "s"}` : "Not found anywhere in the game.";
  out.replaceChildren(
    ...found.map((f) => {
      const text = el("span", { className: "found-text" }, f.text.slice(0, f.at), el("mark", { textContent: f.text.slice(f.at, f.at + f.length) }), f.text.slice(f.at + f.length));
      const li = el("li", { title: "Go to it" }, el("strong", { textContent: f.where }), f.field ? el("span", { className: "dim", textContent: ` ${f.field}` }) : "", text);
      li.tabIndex = 0;
      const go = () => {
        if (f.line !== undefined) {
          openFile(f.path);
          selectLine(f.line);
        } else {
          goTo(f.path);
          // Showing it on the map or in the panel: stay on the results.
          if ($("panel-yaml").hidden) showTab("find");
        }
      };
      li.addEventListener("click", go);
      li.addEventListener("keydown", (e) => e.key === "Enter" && go());
      return li;
    }),
  );
}
let findTimer = 0;
$("find-text").addEventListener("input", () => {
  clearTimeout(findTimer);
  findTimer = window.setTimeout(runFind, 200);
});
function openFind() {
  showTab("find");
  const input = $<HTMLInputElement>("find-text");
  input.focus();
  input.select();
  runFind();
}
$("tab-find").addEventListener("click", openFind);

/** Opens where a script is: a file under scripts/, or the handler or conversation it belongs to. */
function openScript(origin: string) {
  if (project.files[origin] !== undefined) return openFile(origin);
  goTo(origin);
}

/** Selects a line of the file open in the Files tab. */
function selectLine(line: number) {
  const ta = $<HTMLTextAreaElement>("yaml");
  const lines = ta.value.split("\n");
  const at = lines.slice(0, line - 1).reduce((n, l) => n + l.length + 1, 0);
  ta.focus();
  ta.setSelectionRange(at, at + (lines[line - 1]?.length ?? 0));
  ta.scrollTop = Math.max(0, (line - 3) * parseFloat(getComputedStyle(ta).lineHeight || "18"));
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
      const after = snapshot();
      if (!sameFiles(before.files, after.files)) {
        pushUndo(before, after);
        save();
        scheduleCheck();
      }
      typingYaml = false;
      return;
    }
    try {
      const next = new Model(text);
      const before = snapshot();
      pushUndo(before, { files: { ...before.files, "game.yaml": text } });
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
// A reply from an AI assistant pasted over game.yaml: the code, without the chat around it.
$("yaml").addEventListener("paste", (e) => {
  if (currentFile !== "game.yaml") return;
  const text = e.clipboardData?.getData("text") ?? "";
  if (!/^(`{3,}|~{3,})/m.test(text)) return;
  const reply = fromChat(text);
  if (!reply) return;
  e.preventDefault();
  const ta = $<HTMLTextAreaElement>("yaml");
  ta.value = reply.yaml;
  ta.dispatchEvent(new Event("input"));
  if (reply.walkthrough) {
    const walk = reply.walkthrough;
    change(() => (project = { ...project, files: { ...project.files, [WALKTHROUGH]: walk } }), { label: "Pasted the walkthrough" });
  }
  $("yaml-msg").textContent = `Pasted the game${reply.walkthrough ? ", and its walkthrough," : ""} from the reply.`;
});
$("tab-problems").addEventListener("click", () => showTab("problems"));
$("tab-puzzles").addEventListener("click", () => showTab("puzzles"));
$("tab-play").addEventListener("click", () => showTab("play"));
$("tab-timers").addEventListener("click", () => showTab("timers"));
$("tab-yaml").addEventListener("click", () => openFile(currentFile));
$("file").addEventListener("change", () => openFile($<HTMLSelectElement>("file").value));

// ------------------------------------------------------------ toolbar

$("undo").addEventListener("click", doUndo);
$("redo").addEventListener("click", doRedo);
$("history").addEventListener("click", showHistory);
$("play").addEventListener("click", () => {
  save();
  window.open(`./play.html?local=${encodeURIComponent(project.key)}&fresh=1`, "_blank", "noopener");
});
function downloadBlob(blob: Blob, name: string) {
  const a = el("a", { href: URL.createObjectURL(blob), download: name });
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 1000);
}
const gameId = () => (typeof model?.get(["game", "id"]) === "string" ? (model.get(["game", "id"]) as string) : "game").replace(/[^\w-]/g, "_");

// Download: the game's files as a zip (to keep, or open here again), or
// the game as one web page that plays anywhere (to share).
$("download").addEventListener("click", () => {
  save();
  const dialog = $<HTMLDialogElement>("download-dialog");
  const status = el("p", { className: "dim", role: "status" });
  const errors = problems.filter((p) => p.severity === "error").length;
  const zip = button("Download the files (.zip)", () => {
    const id = gameId();
    downloadBlob(new Blob([zipSync(Object.fromEntries(Object.entries(project.files).map(([p, t]) => [`${id}/${p}`, (isPicture(p, t) && dataBytes(t)) || strToU8(t)])))], { type: "application/zip" }), `${id}.zip`);
    dialog.close();
  });
  const page = button(
    "Download a web page (.html)",
    async () => {
      page.disabled = true;
      status.textContent = "Putting the page together…";
      try {
        const { fetchEngineFiles, standalonePage } = await import("../standalone.ts");
        const engineFiles = await fetchEngineFiles(new URL("./wasm/", location.href));
        const site = location.protocol === "https:" ? new URL("./", location.href).href : undefined;
        const html = standalonePage({ ...project.files }, model?.title() ?? "My Game", engineFiles, site);
        downloadBlob(new Blob([html], { type: "text/html" }), `${gameId()}.html`);
        dialog.close();
      } catch (e) {
        status.textContent = `The page couldn't be made: ${e}`;
        page.disabled = false;
      }
    },
    "primary",
  );
  page.disabled = errors > 0;
  dialog.replaceChildren(
    button("Cancel", () => dialog.close(), "close"),
    el("h2", { id: "download-title", textContent: "Download" }),
    el("div", { className: "choice-row" }, zip, el("p", { className: "dim", textContent: "Your game's files, to keep or to open in the editor again later (Choose files or a zip, on the editor's first page)." })),
    el(
      "div",
      { className: "choice-row" },
      page,
      el("p", {
        className: "dim",
        textContent: errors
          ? "Fix the game's errors (under Problems) first: a page needs a game that can be played."
          : "Your game as a single page that anyone can play in a browser, even offline: put it on any website, or send it to someone. It's about 3 MB, as the game engine is inside it.",
      }),
    ),
    status,
  );
  dialog.showModal();
});
// The editor theme: the map in the site's colours, or as a blueprint. Remembered in this browser.
function showMapTheme(t: MapTheme) {
  document.querySelector(".canvas")?.classList.toggle("blueprint", t === "blueprint");
  $<HTMLSelectElement>("map-theme").value = t;
}
showMapTheme(mapTheme.get());
$("map-theme").addEventListener("change", () => {
  const t = $<HTMLSelectElement>("map-theme").value === "blueprint" ? "blueprint" : "site";
  mapTheme.set(t);
  showMapTheme(t);
});
$("add-component").addEventListener("click", () => {
  if (!model) return;
  openAddComponent($<HTMLDialogElement>("add-dialog"), {
    ...inspectorContext(model),
    addRoom: () => {
      const r = $("map").getBoundingClientRect();
      addAt(mapView.toMap(r.left + r.width / 2, r.top + r.height / 2));
    },
  });
});
$("zoom-in").addEventListener("click", () => mapView.zoom(1.25));
$("zoom-out").addEventListener("click", () => mapView.zoom(0.8));
$("fit").addEventListener("click", () => mapView.fit());
$("tidy").addEventListener("click", () => {
  if (!model) return;
  change((m) => (positions = tidy(m, positions)), { label: "Tidied up the map" });
  mapView.fit();
});

document.addEventListener("keydown", (e) => {
  const typing = e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLSelectElement;
  if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === "f" && model) {
    e.preventDefault();
    openFind();
  } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "z" && !typing) {
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
    } else if (selection.kind === "ending" || selection.kind === "timer" || selection.kind === "hint" || selection.kind === "goal" || selection.kind === "choice" || selection.kind === "verb") {
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

const talkView = new TalkView(document.getElementById("talk") as unknown as SVGSVGElement, {
  pick: (node) => {
    talkPicked = node;
    drawTalk();
    // The part, in the details panel.
    const card = $("inspector").querySelector<HTMLElement>(`[data-node="${CSS.escape(node)}"]`);
    card?.scrollIntoView({ block: "start", behavior: "smooth" });
    card?.classList.remove("flash");
    void card?.offsetWidth;
    card?.classList.add("flash");
  },
  move: (node, to) => {
    if (selection?.kind !== "choice") return;
    const id = selection.id;
    change(
      () => {
        const m = talkPos.get(id) ?? new Map<string, Point>();
        // Keep the other parts where they're drawn now, so they don't jump.
        const c = model?.choice(id);
        if (c) for (const [n, p] of talkLayout(c, m)) if (!m.has(n)) m.set(n, p);
        m.set(node, to);
        talkPos.set(id, m);
      },
      { coalesce: `talk:${id}:${node}`, label: "Moved a part of the conversation", redraw: false },
    );
  },
  link: (node, option, to) => {
    if (selection?.kind !== "choice") return;
    const id = selection.id;
    if ("node" in to) {
      change((m) => m.setOptionNext(id, node, option, { goto: to.node }), { label: `Made a reply lead to the part “${to.node}”` });
      return;
    }
    change(
      (m) => {
        const fresh = m.addNode(id);
        const pos = talkPos.get(id) ?? new Map<string, Point>();
        const c = m.choice(id);
        if (c) for (const [n, p] of talkLayout(c, pos)) if (!pos.has(n)) pos.set(n, p);
        pos.set(fresh, to.at);
        talkPos.set(id, pos);
        m.setOptionNext(id, node, option, { goto: fresh });
        talkPicked = fresh;
      },
      { label: "Added a part to the conversation" },
    );
  },
});
$("talk-back").addEventListener("click", () => select(null));

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
  // Starter templates: small games to start from instead of a blank one.
  fetch("./games/templates.json")
    .then((r) => r.json())
    .then((starters: { id: string; title: string; about: string; files: Files }[]) => {
      $("starters").append(
        ...starters.map((t) =>
          el(
            "li",
            {},
            el("span", { className: "name", textContent: t.title }),
            el("span", { className: "dim", textContent: t.about }),
            button("Start from this", () => {
              const g: LocalGame = { key: newKey(), title: t.title, files: { ...t.files }, added: Date.now() };
              if (!localGames.put(g)) return void ($("open-msg").textContent = "There's no room left in this browser's storage.");
              open(g);
            }),
          ),
        ),
      );
    })
    .catch(() => {
      /* the blank game is still there */
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
  // Made with an AI assistant: the instructions to give it, and its reply to open.
  $("ai-copy").addEventListener("click", async () => {
    const guide = await fetch("./docs/ai.md")
      .then((r) => (r.ok ? r.text() : null))
      .catch(() => null);
    const ok = await copyText(instructions(guide));
    $("ai-msg").textContent = ok
      ? "Copied, with the guide to the format. Paste it into a new chat, and describe your story where it says."
      : "Couldn't copy here. Tell your AI to read https://ravzac14.github.io/EIFGB/docs/ai.md and reply with one game.yaml.";
    $("ai-msg").classList.toggle("err", !ok);
  });
  $("ai-open").addEventListener("click", () => {
    const reply = fromChat($<HTMLTextAreaElement>("ai-reply").value);
    if (!reply) {
      $("ai-msg").textContent = "There's no game in that: paste the reply with the block that starts format: eifgb/0.1.";
      $("ai-msg").classList.add("err");
      return;
    }
    let title = "Untitled game";
    try {
      title = new Model(reply.yaml).title();
    } catch {
      /* opens on the file tab, with what's wrong */
    }
    const files: Files = { "game.yaml": reply.yaml, ...(reply.walkthrough ? { [WALKTHROUGH]: reply.walkthrough } : {}) };
    const g: LocalGame = { key: newKey(), title, files, added: Date.now() };
    if (!localGames.put(g)) return void ($("ai-msg").textContent = "There's no room left in this browser's storage.");
    open(g);
  });
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
