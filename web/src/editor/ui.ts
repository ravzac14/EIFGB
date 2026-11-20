// Shared pieces for the editor's panels: the context every form gets, and
// small builders for labelled fields, checkboxes and lists.

import { slug, type Direction, type Model } from "./model.ts";
import type { Selection } from "./map.ts";
import type { Diagnostic } from "../engine.ts";

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
  /** The latest problems from the checker. */
  problems: Diagnostic[];
  /** Standard verbs, for the script picker. */
  verbs: { id: string; example: string }[];
  /** Opens a script file in the Files tab. */
  openFile(path: string): void;
  /** Adds a script file to the game; returns its path. */
  addScriptFile(name: string): string | null;
  /** Adds a picture file to the game (made small); returns its path. */
  addPicture(file: File, base: string): Promise<string | null>;
  /** Where to show a picture from: the project's copy, or its web address. */
  pictureUrl(file: string): string | null;
  /** Deletes a picture file nothing uses any more. */
  dropPicture(file: string): void;
  /** A room's colour on the map (kept in map.yaml), or undefined. */
  roomColour(id: string): string | undefined;
  setRoomColour(id: string, colour: string | null): void;
}

type Kids = (Node | string)[];
export function el<K extends keyof HTMLElementTagNameMap>(tag: K, props: Record<string, unknown> = {}, ...kids: Kids) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(props)) {
    if (k === "class") e.className = String(v);
    else if (k === "ariaLabel") e.setAttribute("aria-label", String(v));
    else if (k.startsWith("on") && typeof v === "function") e.addEventListener(k.slice(2), v as EventListener);
    else if (v !== undefined && v !== false) (e as unknown as Record<string, unknown>)[k] = v;
  }
  e.append(...kids);
  return e;
}
export const button = (text: string, onclick: () => void, cls = "") => el("button", { type: "button", textContent: text, class: cls, onclick });
export const help = (text: string) => el("p", { class: "help", textContent: text });
let fieldCount = 0;

/** A labelled text field (or textarea) that edits as you type. */
export function field(
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

export function check(label: string, checked: boolean, onchange: (v: boolean) => void) {
  const box = el("input", { type: "checkbox", checked });
  box.addEventListener("change", () => onchange((box as HTMLInputElement).checked));
  return el("label", { class: "check" }, box, label);
}

export function select(options: [string, string][], value: string, onchange: (v: string) => void, label?: string) {
  const s = el("select", label ? { ariaLabel: label } : {}) as HTMLSelectElement;
  for (const [v, t] of options) {
    const o = document.createElement("option");
    o.value = v;
    o.textContent = t;
    o.selected = v === value;
    s.append(o);
  }
  // Set as well as marked: an option marked before the list is on the page isn't always honoured.
  if (options.some(([v]) => v === value)) s.value = value;
  s.addEventListener("change", () => onchange(s.value));
  return s;
}

export const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
export const roomName = (m: Model, id?: string) => (id ? (m.room(id)?.name ?? id) : "nowhere");

/** A whole-number field at `path`; empty removes it. */
export function numField(ctx: InspectorContext, path: (string | number)[], label: string, value: unknown, helpText: string, placeholder = "") {
  const i = el("input", { type: "text", inputMode: "numeric", value: typeof value === "number" ? String(value) : "", placeholder }) as HTMLInputElement;
  i.addEventListener("input", () => {
    const n = Number.parseInt(i.value, 10);
    ctx.change((x) => x.set(path, Number.isFinite(n) && n >= 0 ? n : undefined), { coalesce: path.join("."), redraw: false });
  });
  return [el("label", { textContent: label }), i, help(helpText)];
}

/** A checkbox for a true/false field with a default. */
export function boolField(ctx: InspectorContext, path: (string | number)[], label: string, byDefault: boolean, helpText?: string) {
  const c = check(label, ctx.model.bool(path, byDefault), (v) => ctx.change((x) => x.setBool(path, v, byDefault)));
  return helpText ? [c, help(helpText)] : [c];
}

/**
 * Whether an ID was made from a name and never changed by hand: "new_room"
 * for "New room", or "new_room_2". Such IDs follow the name when it changes.
 */
export function idFollows(id: string, name: string, fallback: string): boolean {
  const base = slug(name, fallback);
  return id === base || new RegExp(`^${base}_\\d+$`).test(id);
}
