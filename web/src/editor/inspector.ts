// The details panel: forms for whatever is selected on the map (the game
// itself when nothing is), in plain words.

import { DIRECTIONS, OPPOSITE, type Direction, type Exit, type Model } from "./model.ts";
import type { Selection } from "./map.ts";

export interface InspectorContext {
  model: Model;
  selection: Selection;
  /**
   * Makes an edit. `coalesce` merges typing in one field into one undo step;
   * `redraw: false` leaves the panel alone (so a field being typed in keeps focus).
   */
  change(fn: (m: Model) => void, opts?: { coalesce?: string; redraw?: boolean }): void;
  select(sel: Selection): void;
  /** Adds a room `dir` of `from`, connected both ways; returns its ID. */
  addRoomBeside(from: string, dir: Direction): string;
  deleteRoom(id: string): void;
  /** Opens the game file at this text. */
  showInFile(find: string): void;
}

type Kids = (Node | string)[];
function el<K extends keyof HTMLElementTagNameMap>(tag: K, props: Record<string, unknown> = {}, ...kids: Kids) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(props)) {
    if (k === "class") e.className = String(v);
    else if (k.startsWith("on") && typeof v === "function") e.addEventListener(k.slice(2), v as EventListener);
    else if (v !== undefined && v !== false) (e as unknown as Record<string, unknown>)[k] = v;
  }
  e.append(...kids);
  return e;
}
const button = (text: string, onclick: () => void, cls = "") => el("button", { type: "button", textContent: text, class: cls, onclick });
const help = (text: string) => el("p", { class: "help", textContent: text });
let fieldCount = 0;

/** A labelled text field (or textarea) that edits as you type. */
function field(label: string, value: string, oninput: (v: string) => void, opts: { multiline?: boolean; placeholder?: string; help?: string } = {}) {
  const id = `f${++fieldCount}`;
  const input = opts.multiline
    ? el("textarea", { id, value, placeholder: opts.placeholder ?? "" })
    : el("input", { id, type: "text", value, placeholder: opts.placeholder ?? "" });
  input.addEventListener("input", () => oninput((input as HTMLInputElement).value));
  const out: HTMLElement[] = [el("label", { htmlFor: id, textContent: label }), input];
  if (opts.help) out.push(help(opts.help));
  return out;
}

function check(label: string, checked: boolean, onchange: (v: boolean) => void) {
  const box = el("input", { type: "checkbox", checked });
  box.addEventListener("change", () => onchange((box as HTMLInputElement).checked));
  return el("label", { class: "check" }, box, label);
}

function select(options: [string, string][], value: string, onchange: (v: string) => void, label?: string) {
  const s = el("select", label ? { ariaLabel: label } : {}) as HTMLSelectElement;
  for (const [v, t] of options) {
    const o = document.createElement("option");
    o.value = v;
    o.textContent = t;
    o.selected = v === value;
    s.append(o);
  }
  s.addEventListener("change", () => onchange(s.value));
  return s;
}

const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
const roomName = (m: Model, id?: string) => (id ? (m.room(id)?.name ?? id) : "nowhere");

export function renderInspector(panel: HTMLElement, ctx: InspectorContext) {
  panel.replaceChildren();
  const s = ctx.selection;
  if (s?.kind === "room" && ctx.model.room(s.id)) return roomPanel(panel, ctx, s.id);
  if (s?.kind === "exit" && ctx.model.exit(s.room, s.dir)) return exitPanel(panel, ctx, s.room, s.dir);
  gamePanel(panel, ctx);
}

// ------------------------------------------------------------- the game

function gamePanel(panel: HTMLElement, ctx: InspectorContext) {
  const m = ctx.model;
  const str = (p: string[]) => {
    const v = m.get(p);
    return typeof v === "string" ? v : "";
  };
  panel.append(
    el("p", { class: "kind", textContent: "Your game" }),
    el("h2", { textContent: m.title() }),
    ...field("Title", str(["game", "title"]), (v) => ctx.change((x) => x.set(["game", "title"], v), { coalesce: "title", redraw: false })),
    ...field("Author", str(["game", "author"]), (v) => ctx.change((x) => x.set(["game", "author"], v || undefined), { coalesce: "author", redraw: false }), {
      placeholder: "Your name (optional)",
    }),
    ...field("Introduction", str(["game", "intro"]), (v) => ctx.change((x) => x.set(["game", "intro"], v || undefined), { coalesce: "intro", redraw: false }), {
      multiline: true,
      help: "Shown once, when the game starts, before the first room.",
    }),
    el("label", { textContent: "The story starts in" }),
    select(
      m.rooms().map((r) => [r.id, r.name]),
      m.start() ?? "",
      (v) => ctx.change((x) => x.set(["game", "start"], v)),
      "Starting room",
    ),
    el("h3", { textContent: "Drawing the map" }),
    el(
      "ul",
      { class: "tips" },
      el("li", { textContent: "Click a room to select it. Click one of the + around it to add a room that way." }),
      el("li", { textContent: "Drag a + onto another room to connect the two." }),
      el("li", { textContent: "Drag rooms to arrange the map; drag the background to move around." }),
      el("li", { textContent: "Click a line to choose its door, or to make it one way." }),
      el("li", { textContent: "Double-click empty space for a new room on its own." }),
    ),
  );
}

// ------------------------------------------------------------- a room

function exitLine(ctx: InspectorContext, room: string, e: Exit) {
  const m = ctx.model;
  const what =
    e.kind === "blocked"
      ? `blocked: “${e.blocked}”`
      : e.kind === "advanced"
        ? `${e.to ? `to ${roomName(m, e.to)}, ` : ""}with conditions`
        : `to ${roomName(m, e.to)}${e.door ? `, through the ${m.door(e.door)?.name ?? e.door}` : ""}`;
  return el(
    "li",
    {},
    el("span", { class: "what" }, el("span", { class: "dir", textContent: cap(e.dir) }), ` ${what}`),
    button("Details", () => ctx.select({ kind: "exit", room, dir: e.dir })),
  );
}

function roomPanel(panel: HTMLElement, ctx: InspectorContext, id: string) {
  const m = ctx.model;
  const r = m.room(id)!;
  const exits = m.exits(id);
  const free = DIRECTIONS.filter((d) => !exits.some((e) => e.dir === d));
  panel.append(
    el("p", { class: "kind", textContent: "Room" }),
    el("h2", { textContent: r.name }),
    ...field("Name", r.name, (v) => ctx.change((x) => x.set(["rooms", id, "name"], v || id), { coalesce: `name:${id}`, redraw: false }), {
      help: "The room's title, shown when the player arrives.",
    }),
    ...field("Description", r.description, (v) => ctx.change((x) => x.set(["rooms", id, "description"], v || undefined), { coalesce: `desc:${id}`, redraw: false }), {
      multiline: true,
      placeholder: "What the player sees here.",
      help: "Exits, doors and things in the room are listed after this automatically.",
    }),
    check("The story starts here", m.start() === id, (v) => {
      if (v) ctx.change((x) => x.set(["game", "start"], id));
      else ctx.change(() => {}); // a game needs a start: keep it, and redraw
    }),
    check("It's dark: the player needs a light to see", r.dark, (v) => ctx.change((x) => x.set(["rooms", id, "dark"], v || undefined))),
    el("h3", { textContent: "Ways out" }),
  );
  if (exits.length) panel.append(el("ul", { class: "exits" }, ...exits.map((e) => exitLine(ctx, id, e))));
  else panel.append(help("No ways out yet. Add one below, or with the + around the room on the map."));

  // Add an exit: a direction and where it goes.
  if (free.length) {
    let dir: Direction = free[0];
    let to = "__new";
    const dirSel = select(free.map((d) => [d, cap(d)]), dir, (v) => (dir = v as Direction), "Direction");
    const others = m.rooms().filter((x) => x.id !== id);
    const toSel = select(
      [["__new", "A new room"], ...others.map((x): [string, string] => [x.id, x.name]), ["__blocked", "Nowhere: it's blocked"]],
      to,
      (v) => (to = v),
      "Leads to",
    );
    const add = button("Add", () => {
      if (to === "__new") {
        const nid = ctx.addRoomBeside(id, dir);
        ctx.select({ kind: "room", id: nid });
      } else if (to === "__blocked") {
        ctx.change((x) => x.setBlocked(id, dir, "You can't go that way."));
        ctx.select({ kind: "exit", room: id, dir });
      } else {
        ctx.change((x) => x.connect(id, dir, to));
      }
    });
    panel.append(el("div", { class: "add" }, dirSel, toSel, add), help("Up, down, in and out are here too. A new way gets a way back, when that direction is free."));
  }

  // Advanced: the ID, and deleting.
  const idInput = el("input", { type: "text", value: id, ariaLabel: "Room ID" }) as HTMLInputElement;
  const msg = help("");
  const rename = button("Rename", () => {
    let next: string | null = null;
    ctx.change((x) => {
      next = x.renameRoom(id, idInput.value);
    });
    if (next) ctx.select({ kind: "room", id: next });
    else msg.textContent = "That ID is already used by something else.";
  });
  const inside = m.thingsIn(id);
  panel.append(
    el(
      "details",
      {},
      el("summary", { textContent: "More" }),
      el("label", { textContent: "ID" }),
      el("div", { class: "row" }, idInput, rename),
      help("How scripts and the game file refer to this room. Renaming updates the exits and everything placed here, but not Lua scripts."),
      msg,
      el(
        "div",
        { class: "actions" },
        button(
          "Delete this room",
          () => {
            const warn = inside.length ? `\n\n${inside.length} thing${inside.length > 1 ? "s" : ""} in it will be taken off the map (not deleted).` : "";
            if (confirm(`Delete ${r.name}, and the ways into it?${warn}`)) ctx.deleteRoom(id);
          },
          "danger",
        ),
      ),
    ),
  );
}

// ------------------------------------------------------------- an exit

function exitPanel(panel: HTMLElement, ctx: InspectorContext, room: string, dir: Direction) {
  const m = ctx.model;
  const e = m.exit(room, dir)!;
  const from = roomName(m, room);
  panel.append(el("p", { class: "kind", textContent: "Way out" }));

  if (e.kind === "advanced") {
    panel.append(
      el("h2", { textContent: `${from}, going ${dir}` }),
      el("p", { class: "note", textContent: "This way out has conditions, or leads to different places at different times. The map shows it; change it in the game file." }),
      el("div", { class: "actions" }, button("Show it in the game file", () => ctx.showInFile(`${room}:`)), button("Remove it", () => removeExit(ctx, room, dir), "danger")),
    );
    return;
  }

  if (e.kind === "blocked") {
    panel.append(
      el("h2", { textContent: `${from}, going ${dir}` }),
      help("The player can't go this way. They're told:"),
      ...field("Message", e.blocked ?? "", (v) => ctx.change((x) => x.setBlocked(room, dir, v), { coalesce: `blocked:${room}:${dir}`, redraw: false })),
      el("div", { class: "actions" }, button("Remove it", () => removeExit(ctx, room, dir), "danger")),
    );
    return;
  }

  const to = e.to!;
  const back = m.wayBack(room, to, dir);
  panel.append(el("h2", { textContent: `${from} ⇄ ${roomName(m, to)}` }));
  const there = el("p", {}, `Going `, el("strong", { textContent: dir }), ` from ${from} leads to ${roomName(m, to)}.`);
  panel.append(there);
  if (back) {
    panel.append(el("p", {}, `Going `, el("strong", { textContent: back.dir }), ` from ${roomName(m, to)} comes back.`));
  } else if (e.door) {
    panel.append(help(`The way back, through the door, is added automatically (going ${OPPOSITE[dir]} from ${roomName(m, to)}).`));
  } else {
    const free = !m.exit(to, OPPOSITE[dir]);
    panel.append(el("p", { textContent: "It's one way: there's no way back." }));
    if (free) panel.append(el("div", { class: "actions" }, button(`Add the way back (${OPPOSITE[dir]})`, () => ctx.change((x) => x.connect(room, dir, to)))));
  }

  // The door.
  panel.append(el("h3", { textContent: "Door" }));
  if (!e.door) {
    panel.append(
      help("Nothing in the way. A door can be opened, closed, and locked with a key."),
      el("div", { class: "actions" }, button("Put a door here", () => ctx.change((x) => x.addDoor(room, dir)))),
    );
  } else {
    doorForm(panel, ctx, e.door);
  }

  panel.append(
    el(
      "div",
      { class: "actions" },
      ...(back && !e.door ? [button("Make it one way", () => ctx.change((x) => x.removeExit(to, back.dir))) ] : []),
      button("Remove this way", () => removeExit(ctx, room, dir), "danger"),
    ),
  );
}

function removeExit(ctx: InspectorContext, room: string, dir: Direction) {
  const m = ctx.model;
  const e = m.exit(room, dir);
  const back = e?.to ? m.wayBack(room, e.to, dir) : undefined;
  ctx.change((x) => {
    x.removeExit(room, dir);
    // A two-way route goes as a whole; a door on it goes too.
    if (back && e?.to) x.removeExit(e.to, back.dir);
    if (e?.door && !x.roomIds().some((r) => x.exits(r).some((y) => y.door === e.door))) x.removeDoor(e.door);
  });
  ctx.select({ kind: "room", id: room });
}

function doorForm(panel: HTMLElement, ctx: InspectorContext, id: string) {
  const m = ctx.model;
  const d = m.door(id);
  if (!d) {
    panel.append(help(`It goes through ${id}, which isn't a door.`));
    return;
  }
  const state: "open" | "closed" | "locked" = d.locked ? "locked" : d.open ? "open" : "closed";
  const name = `door-${id}`;
  const radio = (v: typeof state, text: string) => {
    const r = el("input", { type: "radio", name, value: v, checked: state === v });
    r.addEventListener("change", () =>
      ctx.change((x) => {
        x.setDoorState(id, v);
        x.tidyState(["objects", id]);
      }),
    );
    return el("label", {}, r, text);
  };
  panel.append(
    ...field("Name", d.name, (v) => ctx.change((x) => x.set(["objects", id, "name"], v || "door"), { coalesce: `dname:${id}`, redraw: false }), {
      help: "What players call it: OPEN THE OAK DOOR.",
    }),
    ...field("Description", d.description, (v) => ctx.change((x) => x.set(["objects", id, "description"], v || undefined), { coalesce: `ddesc:${id}`, redraw: false }), {
      multiline: true,
      placeholder: "What the player sees when they examine it.",
    }),
    el("label", { textContent: "At the start it's" }),
    el("div", { class: "radio" }, radio("closed", "Closed"), radio("open", "Open"), radio("locked", "Locked")),
  );
  if (d.locked || d.lockable) {
    const keys = m.keysAvailable();
    panel.append(
      el("label", { textContent: "Unlocked with" }),
      select(
        [["", "Nothing (a script unlocks it)"], ...keys.map((k): [string, string] => [k.id, k.name])],
        d.key ?? "",
        (v) => ctx.change((x) => x.set(["objects", id, "unlocks_with"], v || undefined)),
        "Key",
      ),
      help(keys.length ? "Things the player can carry." : "There's nothing the player can carry yet. Objects come in the next part of the editor; until then, add a key in the game file."),
    );
  }
  panel.append(
    check("Secret: hidden until the player finds it", d.hidden, (v) =>
      ctx.change((x) => {
        x.set(["objects", id, "state", "hidden"], v || undefined);
        x.tidyState(["objects", id]);
      }),
    ),
  );
  if (d.hidden) panel.append(el("p", { class: "note", textContent: "Players need a way to find it. Add revealed_by to the door in the game file, like revealed_by: search bookcase." }));
  panel.append(el("div", { class: "actions" }, button("Take the door away", () => ctx.change((x) => x.removeDoor(id)), "danger")));
}
