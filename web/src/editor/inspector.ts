// The details panel: forms for whatever is selected on the map (the game
// itself when nothing is), in plain words.

import {
  BEHAVIORS, DIRECTIONS, KINDS, OPPOSITE, PEOPLE, slug,
  type Behavior, type Direction, type Exit, type Kind, type Model, type Person, type Thing,
} from "./model.ts";
import { THING_DRAG, factionHue } from "./map.ts";
import { COLOURS, colourStyle } from "./layout.ts";
import { choicePanel, endingPanel, hintPanel, scriptsSection, storySections, timerPanel, verbPanel } from "./story.ts";
import { boolField, button, cap, check, el, field, help, idFollows, numField, roomName, select, type InspectorContext } from "./ui.ts";

export type { InspectorContext } from "./ui.ts";


export function renderInspector(panel: HTMLElement, ctx: InspectorContext) {
  panel.replaceChildren();
  const s = ctx.selection;
  if (s?.kind === "room" && ctx.model.room(s.id)) return roomPanel(panel, ctx, s.id);
  if (s?.kind === "exit" && ctx.model.exit(s.room, s.dir)) return exitPanel(panel, ctx, s.room, s.dir);
  if (s?.kind === "thing" && ctx.model.thing(s.id)) return thingPanel(panel, ctx, s.id);
  if (s?.kind === "character" && ctx.model.character(s.id)) return characterPanel(panel, ctx, s.id);
  if (s?.kind === "faction" && ctx.model.faction(s.id)) return factionPanel(panel, ctx, s.id);
  if (s?.kind === "ending" && ctx.model.ids("endings").includes(s.id)) return endingPanel(panel, ctx, s.id);
  if (s?.kind === "timer" && ctx.model.ids("timers").includes(s.id)) return timerPanel(panel, ctx, s.id);
  if (s?.kind === "hint" && ctx.model.ids("hints").includes(s.id)) return hintPanel(panel, ctx, s.id);
  if (s?.kind === "choice" && ctx.model.choice(s.id)) return choicePanel(panel, ctx, s.id);
  if (s?.kind === "verb" && ctx.model.ids("verbs").includes(s.id)) return verbPanel(panel, ctx, s.id);
  gamePanel(panel, ctx);
}

/** A row of swatches: the room's colour on the map, to group rooms into regions. */
function colourPicker(ctx: InspectorContext, id: string): HTMLElement {
  const current = ctx.roomColour(id);
  const row = el("div", { class: "swatches", role: "radiogroup", ariaLabel: "Colour on the map" });
  const pick = (name: string | null, label: string, style: string) => {
    const b = el("button", { type: "button", class: `swatch${(current ?? null) === name ? " on" : ""}`, title: label, ariaLabel: label });
    b.setAttribute("role", "radio");
    b.setAttribute("aria-checked", String((current ?? null) === name));
    if (style) b.setAttribute("style", style);
    b.addEventListener("click", () => ctx.setRoomColour(id, name));
    return b;
  };
  row.append(pick(null, "No colour", ""), ...Object.entries(COLOURS).map(([name, c]) => pick(name, c.label, colourStyle(name))));
  return el("div", {}, el("label", { textContent: "Colour on the map" }), row, help("Group rooms into areas: the castle, the forest. Only for the map; players see it on theirs too."));
}

// ------------------------------------------------------------- the game

/** The game's look on the website: `presentation: { theme, accent }`. */
function lookFields(ctx: InspectorContext): HTMLElement[] {
  const m = ctx.model;
  const theme = m.get(["presentation", "theme"]);
  const accent = m.get(["presentation", "accent"]);
  const setLook = (key: "theme" | "accent", v: string | undefined) =>
    ctx.change((x) => {
      x.set(["presentation", key], v || undefined);
      // Nothing left: no empty `presentation: {}`.
      if (x.get(["presentation"]) !== undefined && !x.get(["presentation", "theme"]) && !x.get(["presentation", "accent"])) x.set(["presentation"], undefined);
    });
  const colour = el("input", { type: "color", ariaLabel: "Highlight colour" }) as HTMLInputElement;
  colour.value = typeof accent === "string" && /^#[0-9a-f]{6}$/i.test(accent) ? accent : typeof accent === "string" && /^#[0-9a-f]{3}$/i.test(accent) ? `#${[...accent.slice(1)].map((c) => c + c).join("")}` : "#8a4b14";
  colour.addEventListener("change", () => setLook("accent", colour.value));
  return [
    el("label", { textContent: "How it looks" }),
    select(
      [
        ["", "As each player likes it"],
        ["light", "Light"],
        ["dark", "Dark"],
        ["sepia", "Sepia"],
        ["parchment", "Parchment"],
        ["terminal", "Terminal (green)"],
        ["amber", "Terminal (amber)"],
        ["contrast", "High contrast"],
      ],
      typeof theme === "string" ? theme : "",
      (v) => setLook("theme", v),
      "Colours",
    ),
    el(
      "div",
      { class: "row" },
      el("label", { class: "check" }, colour, " Highlight colour"),
      ...(typeof accent === "string" ? [button("Use the usual colour", () => setLook("accent", undefined))] : []),
    ),
    help("Players see the game this way unless they pick their own colours with Aa."),
  ];
}

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
    ...lookFields(ctx),
    check("List each room's exits after its description", m.bool(["settings", "list_exits"], true), (v) =>
      ctx.change((x) => {
        x.setBool(["settings", "list_exits"], v, true);
        // No empty `settings: {}` left behind.
        const left = x.get(["settings"]);
        if (left && typeof left === "object" && "items" in left && !(left as { items: unknown[] }).items.length) x.set(["settings"], undefined);
      }),
    ),
    help("“Known exits: north and up.” Only the ways that are open to the player right now are listed."),
    el("h3", { textContent: "Carried at the start" }),
    thingTree(ctx, "player", "Nothing yet. Drag things here, or add one."),
    addThingRow(ctx, "player"),
    ...(m.contents(null).length
      ? [el("h3", { textContent: "Not placed anywhere yet" }), help("Scripts can bring these into the game."), thingTree(ctx, null, "")]
      : []),
    el("h3", { textContent: "Sides" }),
    help("Groups of characters that fight together, and who they attack on sight."),
    sidesList(ctx),
    addSideRow(ctx),
  );
  storySections(panel, ctx);
  scriptsSection(panel, ctx, [], "game");
  panel.append(
    el("h3", { textContent: "Drawing the map" }),
    el(
      "ul",
      { class: "tips" },
      el("li", { textContent: "Click a room to select it. Click one of the + around it to add a room that way." }),
      el("li", { textContent: "Drag a + onto another room to connect the two." }),
      el("li", { textContent: "Drag rooms to arrange the map; drag the background to move around." }),
      el("li", { textContent: "Drag a selected room's corner to make its box bigger or smaller, and give it a colour to group it with others. That's only for picturing the map." }),
      el("li", { textContent: "Test play, at the bottom, plays your game right here: from the start, or from the room you've selected." }),
      el("li", { textContent: "Click a line to choose its door, or to make it one way." }),
      el("li", { textContent: "Double-click empty space for a new room on its own." }),
      el("li", { textContent: "Add component, above the map, adds anything else: an object, a character, a door, an ending and more." }),
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
    check("Lists its exits after the description", m.bool(["rooms", id, "list_exits"], m.bool(["settings", "list_exits"], true)), (v) =>
      ctx.change((x) => x.setBool(["rooms", id, "list_exits"], v, x.bool(["settings", "list_exits"], true))),
    ),
    colourPicker(ctx, id),
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
    el("h3", { textContent: "People and creatures" }),
    peopleList(ctx, m.peopleIn(id), "Nobody here yet."),
    addPersonRow(ctx, id),
  );
  scriptsSection(panel, ctx, ["rooms", id], "room");

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
      help(keys.length ? "Things the player can carry." : "There's nothing the player can carry yet. Add a key to a room (choose “A key” in its list), then pick it here."),
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

export const KIND_LABEL: Record<Kind, string> = {
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

  const holder = t.in ? m.character(t.in) : null;
  const forSale = holder && m.behaviorOf(holder) === "merchant";
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
    ...(forSale
      ? numField(ctx, ["objects", id, "price"], `Price (${holder.name} sells it)`, m.get(["objects", id, "price"]), "In the player's money. Empty: not for sale.")
      : []),
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
  if (!has("portable")) panel.append(...trait("Can be pushed to the next room", "pushable", "Too heavy to carry, but PUSH IT NORTH moves it. Stand on it to reach things up high."));
  panel.append(...boolField(ctx, ["objects", id, "high"], "Up high, out of reach", false, "The player can see it (and whatever is on it), but needs to stand on something to reach it."));

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

  scriptsSection(panel, ctx, ["objects", id], "thing");

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
      ...(has("portable") ? num("Damage, as a weapon", "damage", typeof m.get(["objects", id, "damage"]) === "number" ? (m.get(["objects", id, "damage"]) as number) : null, "How hard it hits when the player attacks WITH it.") : []),
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


// ------------------------------------------------------- characters

export const PERSON_LABEL: Record<Person, string> = {
  talker: "Someone to talk to",
  merchant: "A shopkeeper",
  wanderer: "Someone who wanders",
  follower: "A companion who follows you",
  hunter: "An enemy who hunts you",
  guard: "A guard who blocks the way",
  still: "Someone who stays put",
};

const BEHAVIOR_TEXT: Record<Behavior, [string, string]> = {
  stationary: ["Stays put", "Doesn't move or start fights."],
  conversationalist: ["Stays put, to talk to", "Never moves, so the player always knows where to find them."],
  wanderer: ["Wanders about", "Moves to a neighbouring room now and then."],
  merchant: ["Buys and sells", "Trades with the player: BUY, SELL, haggling. Wanders, but never walks off mid-sale."],
  hunter: ["Hunts the player", "Attacks the player on sight, and heads towards them."],
  guard: ["Guards this room", "Attacks the player here, and otherwise stays put."],
  follower: ["Follows the player", "Goes wherever the player goes."],
  defender: ["Keeps to themselves", "Never starts a fight, but fights back at once."],
};

function personLabel(m: Model, id: string) {
  const c = m.character(id)!;
  return `${c.name}${m.hostile(c) ? " ⚔" : ""}`;
}

function peopleList(ctx: InspectorContext, people: { id: string }[], empty: string): HTMLElement {
  const m = ctx.model;
  const ul = el("ul", { class: "things" });
  for (const p of people) {
    const c = m.character(p.id)!;
    const selected = ctx.selection?.kind === "character" && ctx.selection.id === c.id;
    const dot = el("span", { class: "icon", textContent: "●", ariaHidden: "true" });
    if (c.faction) dot.style.color = `hsl(${factionHue(c.faction)} 60% 50%)`;
    const row = el(
      "div",
      { class: `thing${selected ? " selected" : ""}`, draggable: true, title: "Click to edit; drag onto a room on the map to move" },
      dot,
      el("span", { class: "name", textContent: personLabel(m, c.id) }),
      el("span", { class: "tag", textContent: BEHAVIOR_TEXT[m.behaviorOf(c)][0].toLowerCase() }),
    );
    row.addEventListener("click", () => ctx.select({ kind: "character", id: c.id }));
    row.addEventListener("dragstart", (e) => {
      e.dataTransfer?.setData(THING_DRAG, c.id);
      e.dataTransfer?.setData("text/plain", c.name);
    });
    ul.append(el("li", {}, row));
  }
  if (!people.length && empty) ul.append(el("li", { class: "help", textContent: empty }));
  return ul;
}

function addPersonRow(ctx: InspectorContext, room: string | null): HTMLElement {
  let kind: Person = "talker";
  const kinds = select((Object.keys(PEOPLE) as Person[]).map((k) => [k, PERSON_LABEL[k]]), kind, (v) => (kind = v as Person), "Kind of character");
  const add = button("Add", () => {
    let id = "";
    ctx.change((x) => {
      id = x.addCharacter(kind, room);
    });
    ctx.select({ kind: "character", id });
    ctx.focusName();
  });
  return el("div", { class: "add two" }, kinds, add);
}



function characterPanel(panel: HTMLElement, ctx: InspectorContext, id: string) {
  const m = ctx.model;
  const c = m.character(id)!;
  const p = ["characters", id];
  const behavior = m.behaviorOf(c);
  const get = (k: string) => m.get([...p, k]);

  const where = select(
    [...m.rooms().map((r): [string, string] => [r.id, r.name]), ["", "Nowhere yet (a script brings them in)"]],
    c.in ?? "",
    (v) => ctx.change((x) => x.moveCharacter(id, v || null)),
    "Where they start",
  );
  const behaviors = el("div", { class: "radio column" });
  for (const b of BEHAVIORS) {
    const r = el("input", { type: "radio", name: `behavior-${id}`, value: b, checked: behavior === b });
    r.addEventListener("change", () => ctx.change((x) => x.set([...p, "behavior"], b)));
    behaviors.append(el("label", { title: BEHAVIOR_TEXT[b][1] }, r, BEHAVIOR_TEXT[b][0]));
  }

  panel.append(
    el("p", { class: "kind", textContent: "Character" }),
    el("h2", { textContent: personLabel(m, id) }),
    ...field("Name", c.name, (v) => ctx.change((x) => x.set([...p, "name"], v || id), { coalesce: `cname:${id}`, redraw: false }), {
      help: "What players call them: TALK TO THE OLD WOMAN.",
      onchange: (v) => {
        if (!idFollows(id, c.name, "character") || slug(v, "character") === id) return;
        let next: string | null = null;
        ctx.change((x) => (next = x.renameCharacter(id, x.freshId(slug(v, "character")))), { coalesce: `cname:${id}` });
        if (next) ctx.select({ kind: "character", id: next });
      },
    }),
    ...field("Description", c.description, (v) => ctx.change((x) => x.set([...p, "description"], v || undefined), { coalesce: `cdesc:${id}`, redraw: false }), {
      multiline: true,
      placeholder: "What the player sees when they look at them.",
    }),
    ...field("Other words for them", c.synonyms.join(", "), (v) => ctx.change((x) => x.setList([...p, "synonyms"], v.split(",")), { coalesce: `csyn:${id}`, redraw: false }), {
      placeholder: "woman, crone",
    }),
    el("label", { textContent: "Where they start" }),
    where,
    el("h3", { textContent: "What they do" }),
    behaviors,
    help(BEHAVIOR_TEXT[behavior][1]),
  );

  // What their behaviour needs.
  if (behavior === "wanderer" || behavior === "merchant" || behavior === "hunter") {
    panel.append(...numField(ctx, [...p, "wander"], "How often they move on (percent of turns)", get("wander"), "", behavior === "wanderer" ? "50" : "25"));
  }
  if (behavior === "hunter" || behavior === "follower") {
    panel.append(...numField(ctx, [...p, "range"], "Notices the player this many rooms away", get("range"), "Empty: anywhere."));
  }
  if (behavior === "wanderer" || behavior === "merchant" || behavior === "hunter") {
    const area = m.list([...p, "area"]);
    const box = el("div", { class: "radio column" });
    for (const r of m.rooms()) {
      box.append(
        check(r.name, area.includes(r.id), (v) =>
          ctx.change((x) => x.setList([...p, "area"], v ? [...area, r.id] : area.filter((a) => a !== r.id))),
        ),
      );
    }
    panel.append(el("label", { textContent: "Keeps to these rooms" }), box, help("None ticked: anywhere."));
  }
  if (behavior === "merchant") {
    panel.append(
      el("h3", { textContent: "Trading" }),
      ...numField(ctx, [...p, "money"], "Their money", get("money"), "Empty: they can always pay."),
      ...numField(ctx, [...p, "buys_at"], "Pays this percent of the price for what they buy", get("buys_at"), "", "50"),
      ...numField(ctx, [...p, "haggle"], "Goes this far below their prices when haggling (percent)", get("haggle"), "Empty or 0: no haggling."),
      help("Things they carry are for sale: give each a price."),
    );
  }

  // Talking.
  const choices = m.choiceIds();
  panel.append(
    el("h3", { textContent: "Talking" }),
    el("label", { textContent: "TALK TO them starts" }),
    select(
      [["", "Nothing to say"], ...choices.map((ch): [string, string] => [ch, ch.replace(/_/g, " ")]), ["__new", "A new conversation…"]],
      c.talk ?? "",
      (v) => {
        if (v !== "__new") return ctx.change((x) => x.set([...p, "talk"], v || undefined));
        let ch = "";
        ctx.change((x) => {
          ch = x.addChoice(`talk with ${c.name}`);
          x.set([...p, "talk"], ch);
        });
        ctx.select({ kind: "choice", id: ch });
      },
      "Conversation",
    ),
  );
  if (c.talk && m.choice(c.talk)) panel.append(el("div", { class: "actions" }, button("Edit the conversation", () => ctx.select({ kind: "choice", id: c.talk! }))));

  // Their side.
  const sides = m.factionIds();
  const side = select(
    [["", "None"], ...sides.map((f): [string, string] => [f, f]), ["__new", "A new side…"]],
    c.faction ?? "",
    (v) => {
      if (v === "__new") {
        const name = prompt("Name the new side (wolves, the town guard…)");
        if (!name?.trim()) return ctx.change(() => {});
        let f = "";
        ctx.change((x) => {
          f = x.addFaction(name);
          x.set([...p, "faction"], f);
        });
        return;
      }
      ctx.change((x) => x.set([...p, "faction"], v || undefined));
    },
    "Side",
  );
  panel.append(el("h3", { textContent: "Side" }), side);
  if (c.faction && m.faction(c.faction)) panel.append(el("div", { class: "actions" }, button(`Edit the ${c.faction}`, () => ctx.select({ kind: "faction", id: c.faction! }))));

  // Fighting.
  panel.append(
    el("h3", { textContent: "Fighting" }),
    ...numField(ctx, [...p, "health"], "Health", c.health, "Empty: they can't be hurt by fighting."),
    ...numField(ctx, [...p, "damage"], "Damage per hit", c.damage, "", "1"),
    ...boolField(ctx, [...p, "fights_back"], "Fights back when attacked", true),
    ...numField(ctx, [...p, "patience"], "Puts up with this many attacks first", get("patience"), "", ["hunter", "guard", "defender"].includes(behavior) ? "0" : "2"),
    ...boolField(ctx, [...p, "pacifist"], "Never fights at all", behavior === "merchant" || behavior === "conversationalist"),
    ...boolField(ctx, [...p, "invincible"], "Can't be hurt (the story needs them)", false),
  );

  // What they carry.
  panel.append(el("h3", { textContent: behavior === "merchant" ? "For sale" : "Carrying" }), thingTree(ctx, id, "Nothing. Drag things here, or add one."), addThingRow(ctx, id));

  scriptsSection(panel, ctx, p, "character");

  // More.
  const idInput = el("input", { type: "text", value: id, ariaLabel: "Character ID" }) as HTMLInputElement;
  const msg = help("");
  const fears = m.list([...p, "fears"]);
  panel.append(
    el(
      "details",
      {},
      el("summary", { textContent: "More" }),
      ...boolField(ctx, [...p, "obeys"], "Takes orders from the player", false, "TELL REX TO FETCH."),
      ...boolField(ctx, [...p, "nameable"], "The player can name them", false, "CALL THE DOG REX."),
      ...boolField(ctx, [...p, "sees_in_dark"], "Sees in the dark", false),
      ...boolField(ctx, [...p, "tracker"], "Follows the player's tracks", false),
      ...boolField(ctx, [...p, "corpse"], "Leaves a body when defeated", true),
      check("Afraid of fire", fears.includes("fire"), (v) => ctx.change((x) => x.setList([...p, "fears"], v ? [...fears, "fire"] : fears.filter((f) => f !== "fire")))),
      ...numField(ctx, [...p, "morale"], "Runs away below this percent of their health", get("morale"), "Empty: never runs."),
      el("label", { textContent: "ID" }),
      el(
        "div",
        { class: "row" },
        idInput,
        button("Rename", () => {
          let next: string | null = null;
          ctx.change((x) => {
            next = x.renameCharacter(id, idInput.value);
          });
          if (next) ctx.select({ kind: "character", id: next });
          else msg.textContent = "That ID is already used by something else.";
        }),
      ),
      help("How scripts and the game file refer to them. Renaming updates what they carry, but not Lua scripts."),
      msg,
      el(
        "div",
        { class: "actions" },
        button(
          "Delete this character",
          () => {
            if (!confirm(`Delete ${c.name}? What they carry is left where they were.`)) return;
            const back = c.in && m.room(c.in) ? ({ kind: "room", id: c.in } as const) : null;
            ctx.change((x) => x.deleteCharacter(id));
            ctx.select(back);
          },
          "danger",
        ),
      ),
    ),
  );
}

// ------------------------------------------------------------ sides

function sidesList(ctx: InspectorContext): HTMLElement {
  const m = ctx.model;
  const ul = el("ul", { class: "things" });
  for (const f of m.factions()) {
    const dot = el("span", { class: "icon", textContent: "●", ariaHidden: "true" });
    dot.style.color = `hsl(${factionHue(f.id)} 60% 50%)`;
    const enemies = f.hostileTo.map((h) => (h === "player" ? "the player" : h));
    const row = el(
      "div",
      { class: "thing" },
      dot,
      el("span", { class: "name", textContent: f.id }),
      el("span", { class: "tag", textContent: enemies.length ? `attack ${enemies.join(", ")}` : `${f.members.length} member${f.members.length === 1 ? "" : "s"}` }),
    );
    row.addEventListener("click", () => ctx.select({ kind: "faction", id: f.id }));
    ul.append(el("li", {}, row));
  }
  if (!m.factionIds().length) ul.append(el("li", { class: "help", textContent: "No sides yet." }));
  return ul;
}

function addSideRow(ctx: InspectorContext): HTMLElement {
  const name = el("input", { type: "text", placeholder: "wolves, the town guard…", ariaLabel: "New side's name" }) as HTMLInputElement;
  const add = button("Add a side", () => {
    if (!name.value.trim()) return name.focus();
    let id = "";
    ctx.change((x) => {
      id = x.addFaction(name.value);
    });
    ctx.select({ kind: "faction", id });
  });
  return el("div", { class: "add two" }, name, add);
}

function factionPanel(panel: HTMLElement, ctx: InspectorContext, id: string) {
  const m = ctx.model;
  const f = m.faction(id)!;
  const p = ["factions", id];
  const others = m.factionIds().filter((x) => x !== id);
  const attackedBy = m.factions().filter((x) => x.hostileTo.includes(id)).map((x) => x.id);
  const fears = m.list([...p, "fears"]);

  const nameInput = el("input", { type: "text", value: id, ariaLabel: "Side's name" }) as HTMLInputElement;
  const msg = help("");
  panel.append(
    el("p", { class: "kind", textContent: "Side" }),
    el("h2", { textContent: id }),
    el("label", { textContent: "Name" }),
    el(
      "div",
      { class: "row" },
      nameInput,
      button("Rename", () => {
        let next: string | null = null;
        ctx.change((x) => {
          next = x.renameFaction(id, nameInput.value);
        });
        if (next) ctx.select({ kind: "faction", id: next });
        else msg.textContent = "That name is taken.";
      }),
    ),
    msg,
    el("h3", { textContent: "Attacks on sight" }),
    check("The player", f.hostileTo.includes("player"), (v) => ctx.change((x) => x.setHostile(id, "player", v))),
    ...others.map((o) => check(`The ${o}`, f.hostileTo.includes(o), (v) => ctx.change((x) => x.setHostile(id, o, v)))),
  );
  if (!others.length) panel.append(help("Add another side to set them against each other."));
  if (attackedBy.length) panel.append(help(`Attacked on sight by the ${attackedBy.join(" and the ")}.`));

  panel.append(el("h3", { textContent: "Members" }), peopleList(ctx, f.members.map((x) => ({ id: x })), "Nobody yet. Choose this side on a character."));
  panel.append(
    el("h3", { textContent: "Habits" }),
    ...boolField(ctx, [...p, "eats_corpses"], "Eats bodies left lying around", false),
    ...boolField(ctx, [...p, "sees_in_dark"], "Sees in the dark", false),
    check("Afraid of fire", fears.includes("fire"), (v) => ctx.change((x) => x.setList([...p, "fears"], v ? [...fears, "fire"] : fears.filter((q) => q !== "fire")))),
    ...numField(ctx, [...p, "morale"], "Members run away below this percent of their health", m.get([...p, "morale"]), "Empty: they never run."),
    ...numField(ctx, [...p, "patience"], "Turns on the player after this many members are upset", m.get([...p, "patience"]), "", "2"),
    ...field("Their tracks look like", String(m.get([...p, "prints"]) ?? ""), (v) => ctx.change((x) => x.set([...p, "prints"], v || undefined), { coalesce: `prints:${id}`, redraw: false }), {
      placeholder: "paw prints",
    }),
    el(
      "div",
      { class: "actions" },
      button(
        "Delete this side",
        () => {
          if (!confirm(`Delete the side “${id}”? Its members stay, on no side.`)) return;
          ctx.change((x) => x.deleteFaction(id));
          ctx.select(null);
        },
        "danger",
      ),
    ),
  );
}
