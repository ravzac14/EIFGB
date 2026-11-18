// The details panel: forms for whatever is selected on the map (the game
// itself when nothing is), in plain words.

import { DIRECTIONS, KINDS, OPPOSITE, slug, type Direction, type Exit, type Kind, type Model, type Thing } from "./model.ts";
import { THING_DRAG, type Selection } from "./map.ts";

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
  /** Puts the cursor in the panel's first text field, selected. */
  focusName(): void;
  /** A short message in the top bar (a refused drop, say). */
  notice(text: string): void;
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
function field(
  label: string,
  value: string,
  oninput: (v: string) => void,
  opts: { multiline?: boolean; placeholder?: string; help?: string; onchange?: (v: string) => void } = {},
) {
  const id = `f${++fieldCount}`;
  const input = opts.multiline
    ? el("textarea", { id, value, placeholder: opts.placeholder ?? "" })
    : el("input", { id, type: "text", value, placeholder: opts.placeholder ?? "" });
  input.addEventListener("input", () => oninput((input as HTMLInputElement).value));
  if (opts.onchange) input.addEventListener("change", () => opts.onchange!((input as HTMLInputElement).value));
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
  if (s?.kind === "thing" && ctx.model.thing(s.id)) return thingPanel(panel, ctx, s.id);
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
    el("h3", { textContent: "Carried at the start" }),
    thingTree(ctx, "player", "Nothing yet. Drag things here, or add one."),
    addThingRow(ctx, "player"),
    ...(m.contents(null).length
      ? [el("h3", { textContent: "Not placed anywhere yet" }), help("Scripts can bring these into the game."), thingTree(ctx, null, "")]
      : []),
    el("h3", { textContent: "Drawing the map" }),
    el(
      "ul",
      { class: "tips" },
      el("li", { textContent: "Click a room to select it. Click one of the + around it to add a room that way." }),
      el("li", { textContent: "Drag a + onto another room to connect the two." }),
      el("li", { textContent: "Drag rooms to arrange the map; drag the background to move around." }),
      el("li", { textContent: "Click a line to choose its door, or to make it one way." }),
      el("li", { textContent: "Double-click empty space for a new room on its own." }),
      el("li", { textContent: "Drag things in a room's list onto another room on the map, or into a box or onto a table in the list." }),
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
      onchange: (v) => {
        // An automatic ID follows the name, once it's typed.
        if (!idFollows(id, r.name, "room") || slug(v, "room") === id) return;
        let next: string | null = null;
        ctx.change((x) => (next = x.renameRoom(id, x.freshId(slug(v, "room")))), { coalesce: `name:${id}` });
        if (next) ctx.select({ kind: "room", id: next });
      },
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

  panel.append(
    el("h3", { textContent: "In this room" }),
    thingTree(ctx, id, "Nothing here yet."),
    addThingRow(ctx, id),
  );

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

// ------------------------------------------------------------- things

const KIND_LABEL: Record<Kind, string> = {
  carry: "Something to pick up",
  scenery: "Part of the room",
  container: "A container (box, chest)",
  supporter: "A surface (table, shelf)",
  key: "A key",
  light: "A light (lamp, torch)",
};

function icon(t: Thing): string {
  if (t.traits.includes("container")) return "▣";
  if (t.traits.includes("supporter")) return "▭";
  if (t.traits.includes("light_source")) return "✦";
  if (t.traits.includes("portable")) return "•";
  return "◦";
}

/** Things in or on `place`, nested, each draggable onto another holder or a room on the map. */
function thingTree(ctx: InspectorContext, place: string | null, empty: string): HTMLElement {
  const m = ctx.model;
  const list = (at: string | null, depth: number): HTMLElement => {
    const ul = el("ul", { class: depth ? "things nested" : "things" });
    for (const t of m.contents(at)) {
      const selected = ctx.selection?.kind === "thing" && ctx.selection.id === t.id;
      const notes = [t.hidden ? "hidden" : "", t.locked ? "locked" : t.traits.includes("openable") && t.open ? "open" : ""].filter(Boolean);
      const row = el(
        "div",
        { class: `thing${selected ? " selected" : ""}`, draggable: true, title: "Click to edit; drag to move" },
        el("span", { class: "icon", textContent: icon(t), ariaHidden: "true" }),
        el("span", { class: "name", textContent: t.name }),
        ...(notes.length ? [el("span", { class: "tag", textContent: notes.join(", ") })] : []),
      );
      row.addEventListener("click", () => ctx.select({ kind: "thing", id: t.id }));
      row.addEventListener("dragstart", (e) => {
        e.dataTransfer?.setData(THING_DRAG, t.id);
        e.dataTransfer?.setData("text/plain", t.name);
        if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
      });
      // Dropping on a box or table puts it inside or on top; on anything else, beside it.
      dropZone(row, ctx, m.canHold(t.id) ? t.id : at);
      const li = el("li", {}, row);
      if (m.canHold(t.id) && m.contents(t.id).length) li.append(list(t.id, depth + 1));
      ul.append(li);
    }
    return ul;
  };
  const root = list(place, 0);
  if (!root.children.length && empty) root.append(el("li", { class: "help", textContent: empty }));
  const box = el("div", { class: "tree" }, root);
  dropZone(box, ctx, place);
  return box;
}

function dropZone(target: HTMLElement, ctx: InspectorContext, into: string | null) {
  target.addEventListener("dragover", (e) => {
    if (!e.dataTransfer?.types.includes(THING_DRAG)) return;
    e.preventDefault();
    e.stopPropagation();
    target.classList.add("over");
  });
  target.addEventListener("dragleave", () => target.classList.remove("over"));
  target.addEventListener("drop", (e) => {
    const id = e.dataTransfer?.getData(THING_DRAG);
    target.classList.remove("over");
    if (!id) return;
    e.preventDefault();
    e.stopPropagation();
    let ok = true;
    ctx.change((x) => {
      ok = x.moveThing(id, into);
    });
    if (!ok) ctx.notice("It can't go there: that's inside it.");
  });
}

function addThingRow(ctx: InspectorContext, place: string | null): HTMLElement {
  let kind: Kind = "carry";
  const kinds = select(
    (Object.keys(KINDS) as Kind[]).map((k) => [k, KIND_LABEL[k]]),
    kind,
    (v) => (kind = v as Kind),
    "Kind of thing",
  );
  const add = button("Add", () => {
    let id = "";
    ctx.change((x) => {
      id = x.addThing(kind, place);
    });
    ctx.select({ kind: "thing", id });
    ctx.focusName();
  });
  return el("div", { class: "add two" }, kinds, add);
}

/** Every place a thing could start: rooms, the player, and things that hold others (not inside it). */
function placesFor(m: Model, id: string): HTMLSelectElement {
  const t = m.thing(id)!;
  const s = el("select", { ariaLabel: "Where it starts" }) as HTMLSelectElement;
  const opt = (v: string, text: string, parent: HTMLElement = s) => {
    const o = document.createElement("option");
    o.value = v;
    o.textContent = text;
    o.selected = (t.in ?? "") === v;
    parent.append(o);
  };
  const rooms = el("optgroup", { label: "In a room" });
  for (const r of m.rooms()) opt(r.id, r.name, rooms);
  s.append(rooms);
  opt("player", "Carried by the player");
  const inside = (x: string): boolean => {
    for (let at: string | null = x, i = 0; at && i < 100; i++) {
      if (at === id) return true;
      at = m.thing(at)?.in ?? null;
    }
    return false;
  };
  const holders = m.things().filter((x) => m.canHold(x.id) && !inside(x.id));
  if (holders.length) {
    const g = el("optgroup", { label: "In or on something" });
    for (const h of holders) {
      const room = m.roomOf(h.id);
      const where = room === "player" ? "carried" : room ? roomName(m, room) : "nowhere";
      opt(h.id, `${h.traits.includes("supporter") && !h.traits.includes("container") ? "On" : "In"} the ${h.name} (${where})`, g);
    }
    s.append(g);
  }
  opt("", "Nowhere yet (a script brings it in)");
  return s;
}

const REVEAL_VERBS: [string, string][] = [
  ["search", "searches"],
  ["examine", "examines"],
  ["open", "opens"],
  ["move", "moves"],
  ["lift", "lifts"],
  ["push", "pushes"],
  ["pull", "pulls"],
  ["(physical action)", "touches or moves in any way"],
];

function thingPanel(panel: HTMLElement, ctx: InspectorContext, id: string) {
  const m = ctx.model;
  const t = m.thing(id)!;
  const has = (x: string) => t.traits.includes(x);
  const trait = (label: string, name: string, helpText?: string) => {
    const c = check(label, has(name), (v) => ctx.change((x) => x.setTrait(id, name, v)));
    return helpText ? [c, help(helpText)] : [c];
  };
  const state = (label: string, key: string, on: boolean) => check(label, on, (v) => ctx.change((x) => x.setState(id, key, v)));

  const where = placesFor(m, id);
  where.addEventListener("change", () => ctx.change((x) => x.moveThing(id, where.value || null)));

  const how = has("portable") ? "portable" : has("scenery") ? "scenery" : "fixed";
  const radio = (v: "portable" | "scenery" | "fixed", text: string) => {
    const r = el("input", { type: "radio", name: `carry-${id}`, value: v, checked: how === v });
    r.addEventListener("change", () => ctx.change((x) => x.setPortability(id, v)));
    return el("label", {}, r, text);
  };

  panel.append(
    el("p", { class: "kind", textContent: "Thing" }),
    el("h2", { textContent: t.name }),
    ...field("Name", t.name, (v) => ctx.change((x) => x.set(["objects", id, "name"], v || id), { coalesce: `tname:${id}`, redraw: false }), {
      help: "What players call it: TAKE THE GOLD COIN.",
      onchange: (v) => {
        if (!idFollows(id, t.name, "thing") || slug(v, "thing") === id) return;
        let next: string | null = null;
        ctx.change((x) => (next = x.renameThing(id, x.freshId(slug(v, "thing")))), { coalesce: `tname:${id}` });
        if (next) ctx.select({ kind: "thing", id: next });
      },
    }),
    ...field("Description", t.description, (v) => ctx.change((x) => x.set(["objects", id, "description"], v || undefined), { coalesce: `tdesc:${id}`, redraw: false }), {
      multiline: true,
      placeholder: "What the player sees when they examine it.",
    }),
    ...field("Other words for it", t.synonyms.join(", "), (v) => ctx.change((x) => x.setList(["objects", id, "synonyms"], v.split(",")), { coalesce: `tsyn:${id}`, redraw: false }), {
      placeholder: "money, treasure",
      help: "Words in its name already work. Separate others with commas.",
    }),
    el("label", { textContent: "Where it starts" }),
    where,
    el("label", { textContent: "Can the player pick it up?" }),
    el("div", { class: "radio column" }, radio("portable", "Yes"), radio("scenery", "No: it's part of the room, described there"), radio("fixed", "No, but it's listed in the room")),
    el("h3", { textContent: "What it does" }),
    ...trait("Holds things inside", "container"),
    ...trait("Things can go on top", "supporter"),
    ...trait("Opens and closes", "openable"),
  );
  if (has("openable")) {
    panel.append(el("div", { class: "indent" }, state("Starts open", "open", t.open), ...trait("Locks", "lockable")));
    if (has("lockable")) {
      const keys = m.keysAvailable().filter((k) => k.id !== id);
      panel.append(
        el(
          "div",
          { class: "indent" },
          state("Starts locked", "locked", t.locked),
          el("label", { textContent: "Unlocked with" }),
          select(
            [["", "Nothing (a script unlocks it)"], ...keys.map((k): [string, string] => [k.id, k.name])],
            t.key ?? "",
            (v) => ctx.change((x) => x.set(["objects", id, "unlocks_with"], v || undefined)),
            "Key",
          ),
        ),
      );
    }
  }
  if (has("container")) panel.append(...trait("See-through when closed", "transparent", "Players can see what's inside, but not reach it."));
  if (has("container") || has("supporter")) panel.append(...trait("The player can get in or on it", "enterable"));
  panel.append(
    ...trait("Can be worn", "wearable"),
    ...trait("Can be eaten", "edible"),
    ...trait("Gives light", "light_source", "Lights up dark rooms when it's lit."),
  );
  if (has("light_source")) panel.append(el("div", { class: "indent" }, state("Starts lit", "lit", t.lit)));
  panel.append(...trait("Burns", "flammable"), ...trait("Can be smashed", "destructible"));

  // Hidden until found.
  panel.append(el("h3", { textContent: "Hidden" }), state("Hidden until the player finds it", "hidden", t.hidden));
  if (t.hidden) {
    const simple = t.revealedBy.length <= 1 && (t.revealedBy[0] ?? "").trim().split(/\s+(?![^(]*\))/).length <= 2;
    if (!simple) {
      panel.append(el("p", { class: "note", textContent: `Found by: ${t.revealedBy.join("; or ")}. Change that in the game file.` }));
    } else {
      const rule = (t.revealedBy[0] ?? "").trim();
      const m2 = rule.match(/^(\(.*\)|\S+)\s+(\S+)$/);
      let verb = m2?.[1] ?? "search";
      let what = m2?.[2] ?? "";
      const room = m.roomOf(id);
      const near = m.things().filter((x) => x.id !== id && room && m.roomOf(x.id) === room);
      const write = () => ctx.change((x) => x.set(["objects", id, "revealed_by"], what ? `${verb} ${what}` : undefined));
      panel.append(
        el("label", { textContent: "Found when the player" }),
        el(
          "div",
          { class: "add two" },
          select(REVEAL_VERBS, verb, (v) => {
            verb = v;
            write();
          }, "Action"),
          select([["", "choose…"], ...near.map((x): [string, string] => [x.id, `the ${x.name}`])], what, (v) => {
            what = v;
            write();
          }, "Thing"),
        ),
        help(what ? "Then it appears: “You find a small key.”" : "Choose what they have to search or move. Without that, only a script can reveal it."),
      );
    }
  }

  // More: adjectives, size, the ID, deleting.
  const num = (label: string, key: string, value: number | null, helpText: string) => {
    const i = el("input", { type: "text", inputMode: "numeric", value: value === null ? "" : String(value), placeholder: "none" }) as HTMLInputElement;
    i.addEventListener("input", () => {
      const n = Number.parseInt(i.value, 10);
      ctx.change((x) => x.set(["objects", id, key], Number.isFinite(n) && n >= 0 ? n : undefined), { coalesce: `${key}:${id}`, redraw: false });
    });
    return [el("label", { textContent: label }), i, help(helpText)];
  };
  const idInput = el("input", { type: "text", value: id, ariaLabel: "Thing ID" }) as HTMLInputElement;
  const msg = help("");
  panel.append(
    el(
      "details",
      {},
      el("summary", { textContent: "More" }),
      ...field("Describing words", t.adjectives.join(", "), (v) => ctx.change((x) => x.setList(["objects", id, "adjectives"], v.split(",")), { coalesce: `tadj:${id}`, redraw: false }), {
        placeholder: "gold, shiny",
        help: "Help players tell similar things apart: TAKE THE GOLD COIN.",
      }),
      ...num("Size", "size", t.size, "How big it is, from 1 (the default)."),
      ...(has("container") || has("supporter") ? num("Room for", "capacity", t.capacity, "How much fits in or on it, counting sizes. Empty means no limit.") : []),
      el("label", { textContent: "ID" }),
      el(
        "div",
        { class: "row" },
        idInput,
        button("Rename", () => {
          let next: string | null = null;
          ctx.change((x) => {
            next = x.renameThing(id, idInput.value);
          });
          if (next) ctx.select({ kind: "thing", id: next });
          else msg.textContent = "That ID is already used by something else.";
        }),
      ),
      help("How scripts and the game file refer to it. Renaming updates keys and reveal rules, but not Lua scripts."),
      msg,
      el(
        "div",
        { class: "actions" },
        button(
          "Delete this thing",
          () => {
            const inside = m.contents(id).length;
            if (!confirm(`Delete the ${t.name}?${inside ? ` What's in or on it stays where it was.` : ""}`)) return;
            const back = t.in && m.room(t.in) ? ({ kind: "room", id: t.in } as const) : null;
            ctx.change((x) => x.deleteThing(id));
            ctx.select(back);
          },
          "danger",
        ),
      ),
    ),
  );
}

/**
 * Whether an ID was made from a name and never changed by hand: "new_room"
 * for "New room", or "new_room_2". Such IDs follow the name when it changes.
 */
export function idFollows(id: string, name: string, fallback: string): boolean {
  const base = slug(name, fallback);
  return id === base || new RegExp(`^${base}_\\d+$`).test(id);
}
