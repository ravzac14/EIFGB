// "Add component": one place to add anything a game can have. Pick what
// to add, then fill in what it needs (where an object goes, which way a
// door is on…), and it's created and selected, ready to edit.

import { KINDS, PEOPLE, type Direction, type Kind, type Person } from "./model.ts";
import { KIND_LABEL, PERSON_LABEL } from "./inspector.ts";
import { parseValue } from "./story.ts";
import { button, el, select, type InspectorContext } from "./ui.ts";

export interface AddHost extends InspectorContext {
  /** Adds a room in the middle of the map, and selects it. */
  addRoom(): void;
}

interface Component {
  id: string;
  name: string;
  about: string;
  /** The form for what it needs; null adds it straight away. */
  form: ((h: AddHost, done: () => void) => HTMLElement[]) | null;
  add?: (h: AddHost) => void;
}

/** The room to suggest for something new: the selected room, or where the selected thing is, or the start. */
function suggestedRoom(h: AddHost): string {
  const m = h.model;
  const s = h.selection;
  if (s?.kind === "room") return s.id;
  if (s?.kind === "thing" || s?.kind === "character") {
    const r = m.roomOf(s.id);
    if (r && r !== "player") return r;
  }
  return m.start() ?? m.roomIds()[0] ?? "";
}

const field = (label: string, control: HTMLElement) => [el("label", { textContent: label }), control];
const text = (label: string, placeholder: string, value = "") =>
  el("input", { type: "text", placeholder, value, ariaLabel: label }) as HTMLInputElement;

export const COMPONENTS: Component[] = [
  {
    id: "room",
    name: "Room",
    about: "A place the player can be.",
    form: null,
    add: (h) => h.addRoom(),
  },
  {
    id: "object",
    name: "Object",
    about: "A thing in a room: something to pick up, a box, a table, a key, a lamp.",
    form: (h, done) => {
      const rooms = h.model.rooms();
      let kind: Kind = "carry";
      let where = suggestedRoom(h);
      const kinds = select((Object.keys(KINDS) as Kind[]).map((k) => [k, KIND_LABEL[k]]), kind, (v) => (kind = v as Kind), "Kind of object");
      const place = select(
        [...rooms.map((r): [string, string] => [r.id, r.name]), ["player", "Carried by the player at the start"]],
        where,
        (v) => (where = v),
        "Room to add it to",
      );
      const add = button(
        "Add the object",
        () => {
          let id = "";
          h.change((x) => (id = x.addThing(kind, where || null)));
          done();
          h.select({ kind: "thing", id });
          h.focusName();
        },
        "primary",
      );
      return [...field("What kind?", kinds), ...field("Which room is it in?", place), el("div", { class: "row" }, add)];
    },
  },
  {
    id: "character",
    name: "Character",
    about: "Someone (or something) with a mind of its own: to talk to, trade with, follow or fight.",
    form: (h, done) => {
      let kind: Person = "talker";
      let where = suggestedRoom(h);
      const kinds = select((Object.keys(PEOPLE) as Person[]).map((k) => [k, PERSON_LABEL[k]]), kind, (v) => (kind = v as Person), "Kind of character");
      const place = select(
        [...h.model.rooms().map((r): [string, string] => [r.id, r.name]), ["", "Nowhere yet (a script brings them in)"]],
        where,
        (v) => (where = v),
        "Room to add them to",
      );
      const add = button(
        "Add the character",
        () => {
          let id = "";
          h.change((x) => (id = x.addCharacter(kind, where || null)));
          done();
          h.select({ kind: "character", id });
          h.focusName();
        },
        "primary",
      );
      return [...field("What kind?", kinds), ...field("Which room are they in?", place), el("div", { class: "row" }, add)];
    },
  },
  {
    id: "door",
    name: "Door",
    about: "A door on a way between two rooms, to open, close or lock.",
    form: (h, done) => {
      const m = h.model;
      const body = el("div");
      let room = suggestedRoom(h);
      const ways = () => m.exits(room).filter((e) => e.kind === "plain" && e.to);
      const draw = () => {
        const list = ways();
        if (!list.length) {
          body.replaceChildren(el("p", { class: "note", textContent: `${m.room(room)?.name ?? "This room"} has no open way out to put a door on. Connect it to another room on the map first.` }));
          return;
        }
        let dir: Direction = list[0].dir;
        const way = select(
          list.map((e): [string, string] => [e.dir, `${e.dir}, to ${m.room(e.to!)?.name ?? e.to}`]),
          dir,
          (v) => (dir = v as Direction),
          "Which way",
        );
        const name = text("Door's name", "door, gate, trapdoor…", "door");
        const add = button(
          "Add the door",
          () => {
            let id: string | null = null;
            h.change((x) => (id = x.addDoor(room, dir, name.value.trim() || "door")));
            done();
            if (id) h.select({ kind: "exit", room, dir });
          },
          "primary",
        );
        body.replaceChildren(...field("Which way out?", way), ...field("What is it called?", name), el("div", { class: "row" }, add));
      };
      const from = select(
        m.rooms().map((r): [string, string] => [r.id, r.name]),
        room,
        (v) => {
          room = v;
          draw();
        },
        "Room the door is in",
      );
      draw();
      return [...field("In which room?", from), body];
    },
  },
  {
    id: "side",
    name: "Side",
    about: "A group of characters that fight together, and who they attack on sight.",
    form: (h, done) => {
      const name = text("Side's name", "wolves, the town guard…");
      const add = button(
        "Add the side",
        () => {
          if (!name.value.trim()) return name.focus();
          let id = "";
          h.change((x) => (id = x.addFaction(name.value)));
          done();
          h.select({ kind: "faction", id });
        },
        "primary",
      );
      return [...field("What is it called?", name), el("div", { class: "row" }, add)];
    },
  },
  {
    id: "ending",
    name: "Ending",
    about: "A way the story can end: winning, losing, or neither.",
    form: (h, done) => {
      const name = text("Ending's name", "escaped, eaten by wolves…");
      let kind: "win" | "loss" | "neutral" = "win";
      const kinds = select([["win", "The player wins"], ["loss", "The player loses"], ["neutral", "Neither"]], kind, (v) => (kind = v as typeof kind), "Kind of ending");
      const add = button(
        "Add the ending",
        () => {
          let id = "";
          h.change((x) => (id = x.addEnding(name.value.trim() || "the end", kind)));
          done();
          h.select({ kind: "ending", id });
        },
        "primary",
      );
      return [...field("What is it called?", name), ...field("How does it end?", kinds), el("div", { class: "row" }, add)];
    },
  },
  {
    id: "conversation",
    name: "Conversation",
    about: "A conversation or menu, with numbered choices the player types.",
    form: (h, done) => {
      const name = text("Conversation's name", "talk with Meg, the radio…");
      const add = button(
        "Add the conversation",
        () => {
          let id = "";
          h.change((x) => (id = x.addChoice(name.value.trim() || "conversation")));
          done();
          h.select({ kind: "choice", id });
        },
        "primary",
      );
      return [...field("What is it called?", name), el("div", { class: "row" }, add)];
    },
  },
  {
    id: "verb",
    name: "Verb",
    about: "A command of your own: RING BELL, PRAY.",
    form: (h, done) => {
      const word = text("New verb", "ring, dance, pray…");
      const add = button(
        "Add the verb",
        () => {
          if (!word.value.trim()) return word.focus();
          let id = "";
          h.change((x) => (id = x.addVerb(word.value)));
          done();
          h.select({ kind: "verb", id });
        },
        "primary",
      );
      return [...field("The word players type", word), el("div", { class: "row" }, add)];
    },
  },
  {
    id: "timer",
    name: "Timer",
    about: "Something that happens after a number of turns, once or again and again.",
    form: null,
    add: (h) => {
      let id = "";
      h.change((x) => (id = x.addTimer()));
      h.select({ kind: "timer", id });
    },
  },
  {
    id: "variable",
    name: "Variable",
    about: "A value your scripts can read and change: a count, a flag, a name.",
    form: (h, done) => {
      const name = text("Variable's name", "name");
      const value = text("Its starting value", "0, true, or text");
      const add = button(
        "Add the variable",
        () => {
          if (!name.value.trim()) return name.focus();
          h.change((x) => x.addVar(name.value, parseValue(value.value || "0")));
          done();
          h.select(null);
        },
        "primary",
      );
      return [...field("What is it called?", name), ...field("What does it start as?", value), el("div", { class: "row" }, add)];
    },
  },
  {
    id: "script",
    name: "Script file",
    about: "A Lua file, for functions your scripts share.",
    form: (h, done) => {
      const name = text("New script file's name", "helpers");
      const add = button(
        "Add the file",
        () => {
          const path = h.addScriptFile(name.value || "helpers");
          done();
          if (path) h.openFile(path);
        },
        "primary",
      );
      return [...field("What is it called?", name), el("div", { class: "row" }, add)];
    },
  },
];

/** Opens the "Add component" window: the list, then the chosen component's form. */
export function openAddComponent(dialog: HTMLDialogElement, h: AddHost) {
  const done = () => dialog.close();
  const list = () => {
    dialog.replaceChildren(
      button("Cancel", done, "close"),
      el("h2", { textContent: "Add a component" }),
      el(
        "ul",
        { class: "components" },
        ...COMPONENTS.map((c) => {
          const b = el("button", { type: "button", class: "component" }, el("strong", { textContent: c.name }), el("span", { textContent: c.about }));
          b.dataset.component = c.id;
          b.addEventListener("click", () => {
            if (!c.form) {
              done();
              c.add?.(h);
              return;
            }
            // Rooms come first: a game with none has nowhere to put things.
            if (["object", "character", "door"].includes(c.id) && !h.model.roomIds().length) {
              h.notice("Add a room first.");
              return done();
            }
            dialog.replaceChildren(
              button("Cancel", done, "close"),
              el("h2", { textContent: `Add ${c.name === "Script file" ? "a script file" : `a${/^[AEIOU]/.test(c.name) ? "n" : ""} ${c.name.toLowerCase()}`}` }),
              el("p", { class: "dim", textContent: c.about }),
              el("div", { class: "component-form" }, ...c.form(h, done)),
              el("div", { class: "row" }, button("← Something else", list)),
            );
            dialog.querySelector<HTMLElement>("input, select")?.focus();
          });
          return el("li", {}, b);
        }),
      ),
    );
  };
  list();
  if (!dialog.open) dialog.showModal();
  dialog.querySelector<HTMLButtonElement>(".component")?.focus();
}

