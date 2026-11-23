// The map: rooms as boxes, exits as lines, doors on the lines. Drag rooms
// to arrange them; drag from a room's + handles to another room to connect
// them (or into empty space for a new room); click a + for a new room that
// way. Drag the corner of a selected room to resize its box (it's only for
// picturing the map). Drag the background to pan, scroll or use the buttons
// to zoom.

import type { Direction, Exit, Model } from "./model.ts";
import { OPPOSITE } from "./model.ts";
import { boxOf, colourStyle, MAX_BOX, MIN_BOX, type Point, type Positions } from "./layout.ts";

export type Selection =
  | null
  | { kind: "room"; id: string }
  | { kind: "exit"; room: string; dir: Direction }
  | { kind: "thing"; id: string }
  | { kind: "character"; id: string }
  | { kind: "faction"; id: string }
  | { kind: "ending"; id: string }
  | { kind: "timer"; id: string }
  | { kind: "hint"; id: string }
  | { kind: "goal"; id: string }
  | { kind: "choice"; id: string }
  | { kind: "verb"; id: string };

/** The drag-and-drop type for things dragged from the details panel. */
export const THING_DRAG = "application/x-eifgb-thing";

export interface MapActions {
  select(sel: Selection): void;
  moveRoom(id: string, to: Point): void;
  /** Dragged a room's corner: its new box size, or null for the usual size. */
  resizeRoom(id: string, size: { w: number; h: number } | null): void;
  /** Dragged a handle onto a room. */
  connect(from: string, dir: Direction, to: string): void;
  /** Clicked a handle, or dragged it into empty space (`at`). */
  addRoomBeside(from: string, dir: Direction, at?: Point): void;
  addRoomAt(at: Point): void;
  /** A thing or character dragged from the details panel, dropped on a room. */
  dropThing(id: string, room: string): void;
  /** A character's pill dragged to another room. */
  moveCharacter(id: string, room: string): void;
}

/** A side's colour: the same hue every time for the same name. */
export function factionHue(id: string): number {
  let h = 0;
  for (const c of id) h = (h * 31 + c.charCodeAt(0)) % 360;
  return h;
}

const NS = "http://www.w3.org/2000/svg";
const W = 150;
const H = 56;
type Size = { w: number; h: number };
const SHORT: Record<Direction, string> = {
  north: "N", south: "S", east: "E", west: "W", northeast: "NE", northwest: "NW", southeast: "SE", southwest: "SW",
  up: "U", down: "D", in: "IN", out: "OUT",
};
/** Where each compass handle sits on a room's box, as a fraction of half its size. */
const HANDLE: Partial<Record<Direction, [number, number]>> = {
  north: [0, -1], south: [0, 1], east: [1, 0], west: [-1, 0],
  northeast: [1, -1], northwest: [-1, -1], southeast: [1, 1], southwest: [-1, 1],
};
const UNIT: Record<Direction, [number, number]> = {
  north: [0, -1], south: [0, 1], east: [1, 0], west: [-1, 0],
  northeast: [0.7, -0.7], northwest: [-0.7, -0.7], southeast: [0.7, 0.7], southwest: [-0.7, 0.7],
  up: [0.5, -0.86], down: [-0.5, 0.86], in: [0.86, 0.5], out: [-0.86, -0.5],
};

function svg<K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number> = {}, parent?: Element) {
  const e = document.createElementNS(NS, tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
  parent?.append(e);
  return e;
}

/** Where the line from a box's centre towards (dx, dy) leaves the box. */
function edge(c: Point, dx: number, dy: number, pad = 0, size: Size = { w: W, h: H }): Point {
  const hw = size.w / 2 + pad;
  const hh = size.h / 2 + pad;
  const t = Math.min(dx ? hw / Math.abs(dx) : Infinity, dy ? hh / Math.abs(dy) : Infinity);
  return { x: c.x + dx * t, y: c.y + dy * t };
}

interface Link {
  a: string;
  b: string;
  ab?: Exit;
  ba?: Exit;
}

/** Pairs each exit with the exit coming back, so a two-way route is one line. */
function links(model: Model): { links: Link[]; stubs: { room: string; exit: Exit }[] } {
  const rooms = new Set(model.roomIds());
  const out: Link[] = [];
  const stubs: { room: string; exit: Exit }[] = [];
  const used = new Set<string>();
  const all = model.roomIds().flatMap((r) => model.exits(r).map((e) => ({ room: r, exit: e })));
  for (const { room, exit } of all) {
    const k = `${room}:${exit.dir}`;
    if (used.has(k)) continue;
    used.add(k);
    if (!exit.to || !rooms.has(exit.to)) {
      stubs.push({ room, exit });
      continue;
    }
    // The way back: the opposite direction first, else any exit back to here.
    const backs = all.filter((x) => x.room === exit.to && x.exit.to === room && !used.has(`${x.room}:${x.exit.dir}`));
    const back = backs.find((x) => x.exit.dir === OPPOSITE[exit.dir]) ?? backs[0];
    if (back && exit.to !== room) used.add(`${back.room}:${back.exit.dir}`);
    out.push({ a: room, b: exit.to, ab: exit, ba: back && exit.to !== room ? back.exit : undefined });
  }
  return { links: out, stubs };
}

export class MapView {
  private root: SVGGElement;
  private scale = 1;
  private pan = { x: 0, y: 0 };
  private model: Model | null = null;
  private positions: Positions = new Map();
  private selection: Selection = null;
  private bad = new Set<string>();
  /** The room the player is in during test play, marked on the map. */
  private here: string | null = null;
  private svgEl: SVGSVGElement;
  private actions: MapActions;

  constructor(svgEl: SVGSVGElement, actions: MapActions) {
    this.svgEl = svgEl;
    this.actions = actions;
    const defs = svg("defs", {}, svgEl);
    const marker = svg("marker", { id: "arrow", viewBox: "0 0 10 10", refX: 9, refY: 5, markerWidth: 7, markerHeight: 7, orient: "auto-start-reverse" }, defs);
    svg("path", { d: "M0,0 L10,5 L0,10 z" }, marker);
    this.root = svg("g", {}, svgEl);
    this.background();
    this.dropTarget();
  }

  /** Things dragged from the details panel can be dropped on a room. */
  private dropTarget() {
    let over: string | null = null;
    const mark = (id: string | null) => {
      if (id === over) return;
      this.root.querySelector(".drop-target")?.classList.remove("drop-target");
      over = id;
      if (id) this.root.querySelector(`[data-room="${CSS.escape(id)}"]`)?.classList.add("drop-target");
    };
    this.svgEl.addEventListener("dragover", (e) => {
      if (!e.dataTransfer?.types.includes(THING_DRAG)) return;
      const room = this.roomAt(this.toMap(e.clientX, e.clientY));
      mark(room);
      if (room) {
        e.preventDefault();
        e.dataTransfer.dropEffect = "move";
      }
    });
    this.svgEl.addEventListener("dragleave", () => mark(null));
    this.svgEl.addEventListener("drop", (e) => {
      const id = e.dataTransfer?.getData(THING_DRAG);
      const room = this.roomAt(this.toMap(e.clientX, e.clientY));
      mark(null);
      if (id && room) {
        e.preventDefault();
        this.actions.dropThing(id, room);
      }
    });
  }

  // ----------------------------------------------------------- viewing

  private apply() {
    this.root.setAttribute("transform", `translate(${this.pan.x} ${this.pan.y}) scale(${this.scale})`);
  }

  /** Client coordinates → map coordinates. */
  toMap(clientX: number, clientY: number): Point {
    const r = this.svgEl.getBoundingClientRect();
    return { x: (clientX - r.left - this.pan.x) / this.scale, y: (clientY - r.top - this.pan.y) / this.scale };
  }

  zoom(by: number, around?: Point) {
    const r = this.svgEl.getBoundingClientRect();
    const c = around ?? { x: r.width / 2, y: r.height / 2 };
    const next = Math.min(2, Math.max(0.25, this.scale * by));
    this.pan.x = c.x - ((c.x - this.pan.x) * next) / this.scale;
    this.pan.y = c.y - ((c.y - this.pan.y) * next) / this.scale;
    this.scale = next;
    this.apply();
  }

  fit() {
    const r = this.svgEl.getBoundingClientRect();
    const ps = [...this.positions.values()];
    if (!ps.length || !r.width) return;
    const minX = Math.min(...ps.map((p) => p.x)) - W;
    const maxX = Math.max(...ps.map((p) => p.x)) + W;
    const minY = Math.min(...ps.map((p) => p.y)) - H * 1.5;
    const maxY = Math.max(...ps.map((p) => p.y)) + H * 1.5;
    this.scale = Math.min(1.2, Math.max(0.25, Math.min(r.width / (maxX - minX), r.height / (maxY - minY))));
    this.pan.x = r.width / 2 - ((minX + maxX) / 2) * this.scale;
    this.pan.y = r.height / 2 - ((minY + maxY) / 2) * this.scale;
    this.apply();
  }

  /** Scrolls so a room is on screen, if it isn't. */
  reveal(id: string) {
    const p = this.positions.get(id);
    const r = this.svgEl.getBoundingClientRect();
    if (!p || !r.width) return;
    const sx = p.x * this.scale + this.pan.x;
    const sy = p.y * this.scale + this.pan.y;
    if (sx > 40 && sx < r.width - 40 && sy > 40 && sy < r.height - 40) return;
    this.pan.x = r.width / 2 - p.x * this.scale;
    this.pan.y = r.height / 2 - p.y * this.scale;
    this.apply();
  }

  private background() {
    let panning: { x: number; y: number; px: number; py: number; moved: boolean } | null = null;
    this.svgEl.addEventListener("pointerdown", (e) => {
      if (e.target !== this.svgEl) return;
      panning = { x: e.clientX, y: e.clientY, px: this.pan.x, py: this.pan.y, moved: false };
      this.svgEl.setPointerCapture(e.pointerId);
    });
    this.svgEl.addEventListener("pointermove", (e) => {
      if (!panning) return;
      const dx = e.clientX - panning.x;
      const dy = e.clientY - panning.y;
      if (Math.abs(dx) + Math.abs(dy) > 3) panning.moved = true;
      this.pan.x = panning.px + dx;
      this.pan.y = panning.py + dy;
      this.apply();
    });
    this.svgEl.addEventListener("pointerup", () => {
      if (panning && !panning.moved) this.actions.select(null);
      panning = null;
    });
    this.svgEl.addEventListener("dblclick", (e) => {
      if (e.target === this.svgEl) this.actions.addRoomAt(this.toMap(e.clientX, e.clientY));
    });
    this.svgEl.addEventListener(
      "wheel",
      (e) => {
        e.preventDefault();
        const r = this.svgEl.getBoundingClientRect();
        this.zoom(Math.exp(-e.deltaY / 400), { x: e.clientX - r.left, y: e.clientY - r.top });
      },
      { passive: false },
    );
  }

  // ----------------------------------------------------------- drawing

  /** Ways (keyed "a|b") that are hard to read where they're drawn: crossing, say. */
  private flagged = new Set<string>();

  render(model: Model, positions: Positions, selection: Selection, bad: Set<string>, flagged = new Set<string>()) {
    this.flagged = flagged;
    this.model = model;
    this.positions = positions;
    this.selection = selection;
    this.bad = bad;
    this.root.replaceChildren();
    this.linkLayer = svg("g", {}, this.root);
    const roomLayer = svg("g", {}, this.root);
    this.drawLinks();
    for (const id of model.roomIds()) this.drawRoom(roomLayer, id);
    this.peopleLayer = svg("g", {}, this.root);
    for (const id of model.roomIds()) this.drawPeople(this.peopleLayer, id);
    this.handleLayer = svg("g", {}, this.root);
    if (selection?.kind === "room") this.drawHandles(this.handleLayer, selection.id);
    this.apply();
  }

  private linkLayer: SVGGElement | null = null;
  private peopleLayer: SVGGElement | null = null;
  private handleLayer: SVGGElement | null = null;

  private drawLinks() {
    const layer = this.linkLayer!;
    layer.replaceChildren();
    const { links: ls, stubs } = links(this.model!);
    const doors = new Map(this.model!.doors().map((d) => [d.id, d]));
    // Several lines between the same two rooms sit side by side.
    const count = new Map<string, number>();
    for (const l of ls) {
      const k = [l.a, l.b].sort().join("|");
      const n = count.get(k) ?? 0;
      count.set(k, n + 1);
      this.drawLink(layer, l, n, doors);
    }
    for (const s of stubs) this.drawStub(layer, s.room, s.exit);
  }

  private isSelectedExit(room: string, e?: Exit) {
    const s = this.selection;
    return !!e && s?.kind === "exit" && s.room === room && s.dir === e.dir;
  }

  private drawLink(layer: SVGGElement, l: Link, n: number, doors: Map<string, { name: string; locked: boolean; hidden: boolean }>) {
    const pa = this.positions.get(l.a);
    const pb = this.positions.get(l.b);
    if (!pa || !pb) return;
    const selected = this.isSelectedExit(l.a, l.ab) || this.isSelectedExit(l.b, l.ba);
    const advanced = l.ab?.kind === "advanced" || l.ba?.kind === "advanced";
    const flagged = this.flagged.has([l.a, l.b].sort().join("|"));
    const g = svg("g", { class: `link${selected ? " selected" : ""}${advanced ? " advanced" : ""}${flagged ? " flagged" : ""}` }, layer);
    let dx = pb.x - pa.x;
    let dy = pb.y - pa.y;
    if (l.a === l.b) {
      // An exit that loops back to its own room: a small loop above it.
      const [ux, uy] = UNIT[l.ab!.dir];
      dx = ux;
      dy = uy;
    }
    const len = Math.hypot(dx, dy) || 1;
    const off = n * 12;
    const ox = (-dy / len) * off;
    const oy = (dx / len) * off;
    const a = edge({ x: pa.x + ox, y: pa.y + oy }, dx, dy, 4, boxOf(pa));
    const b = l.a === l.b ? edge(pa, dx, dy, 40, boxOf(pa)) : edge({ x: pb.x + ox, y: pb.y + oy }, -dx, -dy, 4, boxOf(pb));
    const door = l.ab?.door ?? l.ba?.door;
    // One-way, unless it goes through a door (the compiler adds the way back).
    const oneWay = !l.ba && !door;
    const attrs = { x1: a.x, y1: a.y, x2: b.x, y2: b.y };
    svg("line", { ...attrs, ...(oneWay ? { "marker-end": "url(#arrow)" } : {}) }, g);
    const hit = svg("line", { ...attrs, class: "hit" }, g);
    const title = svg("title", {}, hit);
    title.textContent = this.describe(l);
    hit.addEventListener("pointerdown", (e) => {
      e.stopPropagation();
      this.actions.select({ kind: "exit", room: l.a, dir: l.ab!.dir });
    });
    // Direction labels by each end.
    const label = (p: Point, toward: Point, e?: Exit) => {
      if (!e) return;
      const t = svg("text", { class: "label", x: p.x + (toward.x - p.x) * 0.18, y: p.y + (toward.y - p.y) * 0.18 - 8 }, g);
      t.textContent = SHORT[e.dir];
    };
    label(a, b, l.ab);
    label(b, a, l.ba);
    if (door) {
      const d = doors.get(door);
      const m = { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
      const dg = svg("g", { class: `door${d?.locked ? " locked" : ""}` }, g);
      svg("rect", { x: m.x - 9, y: m.y - 11, width: 18, height: 22, rx: 3 }, dg);
      const t = svg("text", { x: m.x, y: m.y }, dg);
      t.textContent = d?.locked ? "🔒" : d?.hidden ? "?" : "";
      const tt = svg("title", {}, dg);
      tt.textContent = `${d?.name ?? door}${d?.locked ? " (locked)" : ""}${d?.hidden ? " (secret)" : ""}`;
      dg.addEventListener("pointerdown", (e) => {
        e.stopPropagation();
        this.actions.select({ kind: "exit", room: l.a, dir: l.ab!.dir });
      });
    }
  }

  private describe(l: Link): string {
    const name = (id: string) => this.model?.room(id)?.name ?? id;
    const there = `${name(l.a)} → ${l.ab!.dir} → ${name(l.b)}`;
    return l.ba ? `${there}, and back ${l.ba.dir}` : `${there} (one way)`;
  }

  private drawStub(layer: SVGGElement, room: string, exit: Exit) {
    const p = this.positions.get(room);
    if (!p) return;
    const [ux, uy] = UNIT[exit.dir];
    const a = edge(p, ux, uy, 2, boxOf(p));
    const b = { x: a.x + ux * 34, y: a.y + uy * 34 };
    const g = svg("g", { class: `stub${this.isSelectedExit(room, exit) ? " selected" : ""}` }, layer);
    svg("line", { x1: a.x, y1: a.y, x2: b.x, y2: b.y }, g);
    svg("circle", { cx: b.x, cy: b.y, r: 9 }, g);
    const t = svg("text", { x: b.x, y: b.y }, g);
    t.textContent = exit.kind === "blocked" ? "✕" : "?";
    const hit = svg("line", { x1: a.x, y1: a.y, x2: b.x, y2: b.y, class: "hit" }, g);
    const title = svg("title", {}, hit);
    title.textContent = exit.kind === "blocked" ? `${exit.dir}: “${exit.blocked}”` : `${exit.dir}: an exit with conditions`;
    for (const el of [hit, g])
      el.addEventListener("pointerdown", (e) => {
        e.stopPropagation();
        this.actions.select({ kind: "exit", room, dir: exit.dir });
      });
  }

  /** Marks the room the player is in during test play (null: none). */
  setHere(id: string | null) {
    this.here = id;
    for (const g of this.root.querySelectorAll<SVGGElement>(".room")) g.classList.toggle("here", g.dataset.room === id);
  }

  private drawRoom(layer: SVGGElement, id: string) {
    const p = this.positions.get(id)!;
    const room = this.model!.room(id)!;
    const selected = this.selection?.kind === "room" && this.selection.id === id;
    const g = svg("g", { class: `room${selected ? " selected" : ""}${this.bad.has(id) ? " bad" : ""}${this.here === id ? " here" : ""}`, transform: `translate(${p.x} ${p.y})` }, layer);
    g.dataset.room = id;
    const { w, h } = boxOf(p);
    svg("rect", { x: -w / 2, y: -h / 2, width: w, height: h, rx: 10 }, g);
    if (p.colour) {
      g.setAttribute("style", colourStyle(p.colour));
      g.classList.add("coloured");
    }
    const fits = Math.max(8, Math.floor(w / 7.5));
    const name = room.name.length > fits ? `${room.name.slice(0, fits - 1)}…` : room.name;
    const things = this.model!.contents(id).length;
    const notes = [room.dark ? "dark" : "", things ? `${things} thing${things > 1 ? "s" : ""}` : ""].filter(Boolean).join(" · ");
    const t = svg("text", { x: 0, y: notes ? -6 : 0 }, g);
    t.textContent = name;
    if (notes) {
      const s = svg("text", { class: "sub", x: 0, y: 11 }, g);
      s.textContent = notes;
    }
    if (this.model!.start() === id) {
      const s = svg("text", { class: "start", x: -w / 2 + 8, y: -h / 2 + 10, "text-anchor": "start" }, g);
      s.textContent = "START";
    }
    const title = svg("title", {}, g);
    title.textContent = room.description || room.name;

    // Drag to move; a click selects.
    let drag: { x: number; y: number; ox: number; oy: number; moved: boolean } | null = null;
    g.addEventListener("pointerdown", (e) => {
      e.stopPropagation();
      g.setPointerCapture(e.pointerId);
      drag = { x: e.clientX, y: e.clientY, ox: p.x, oy: p.y, moved: false };
    });
    g.addEventListener("pointermove", (e) => {
      if (!drag) return;
      const dx = (e.clientX - drag.x) / this.scale;
      const dy = (e.clientY - drag.y) / this.scale;
      if (Math.abs(dx) + Math.abs(dy) > 3) drag.moved = true;
      if (!drag.moved) return;
      const snap = (v: number) => Math.round(v / 10) * 10;
      p.x = snap(drag.ox + dx);
      p.y = snap(drag.oy + dy);
      // Move this box and redraw the lines; the element being dragged stays put.
      g.setAttribute("transform", `translate(${p.x} ${p.y})`);
      this.handleLayer?.replaceChildren();
      this.drawLinks();
      this.peopleLayer?.replaceChildren();
      for (const r of this.model!.roomIds()) this.drawPeople(this.peopleLayer!, r);
    });
    // Select on release: selecting redraws the map, which would end a drag.
    g.addEventListener("pointerup", () => {
      const d = drag;
      drag = null;
      if (!d) return;
      if (d.moved) {
        // Put the old position back, so the move is recorded as a change (and can be undone).
        const to = { x: p.x, y: p.y };
        p.x = d.ox;
        p.y = d.oy;
        this.actions.moveRoom(id, to);
      }
      if (!selected || d.moved) this.actions.select({ kind: "room", id });
    });
  }

  /** Characters as small pills under their room: tinted by side, enemies marked. */
  private drawPeople(layer: SVGGElement, room: string) {
    const p = this.positions.get(room);
    if (!p || !this.model) return;
    const people = this.model.peopleIn(room);
    const shown = people.slice(0, 3);
    const widths = shown.map((c) => Math.min(12, c.name.length) * 6.2 + (this.model!.hostile(c) ? 26 : 16));
    const extra = people.length - shown.length;
    const total = widths.reduce((a, b) => a + b, 0) + (shown.length - 1) * 4 + (extra ? 30 : 0);
    let x = p.x - total / 2;
    const y = p.y + boxOf(p).h / 2 + 6;
    shown.forEach((c, i) => {
      const w = widths[i];
      const selected = this.selection?.kind === "character" && this.selection.id === c.id;
      const hostile = this.model!.hostile(c);
      const hue = c.faction ? factionHue(c.faction) : null;
      const g = svg("g", { class: `person${selected ? " selected" : ""}${hostile ? " hostile" : ""}` }, layer);
      const rect = svg("rect", { x, y, width: w, height: 18, rx: 9 }, g);
      if (hue !== null) rect.setAttribute("style", `fill: hsl(${hue} 65% 55% / 0.28)`);
      const label = (hostile ? "⚔ " : "") + (c.name.length > 12 ? `${c.name.slice(0, 11)}…` : c.name);
      const t = svg("text", { x: x + w / 2, y: y + 9.5 }, g);
      t.textContent = label;
      const title = svg("title", {}, g);
      title.textContent = `${c.name}: ${this.model!.behaviorOf(c)}${c.faction ? `, ${c.faction}` : ""}${hostile ? " (attacks the player)" : ""}`;
      this.dragPerson(g, c.id, room);
      x += w + 4;
    });
    if (extra) {
      const t = svg("text", { class: "label", x: x + 14, y: y + 9.5 }, layer);
      t.textContent = `+${extra}`;
    }
  }

  /** Click a pill to select it; drag it to another room to move them. */
  private dragPerson(g: SVGGElement, id: string, room: string) {
    let down: { x: number; y: number } | null = null;
    let ghost: SVGGElement | null = null;
    g.addEventListener("pointerdown", (e) => {
      e.stopPropagation();
      g.setPointerCapture(e.pointerId);
      down = { x: e.clientX, y: e.clientY };
    });
    g.addEventListener("pointermove", (e) => {
      if (!down) return;
      if (!ghost && Math.hypot(e.clientX - down.x, e.clientY - down.y) < 4) return;
      ghost ??= g.cloneNode(true) as SVGGElement;
      ghost.classList.add("ghost");
      this.root.append(ghost);
      const a = this.toMap(down.x, down.y);
      const b = this.toMap(e.clientX, e.clientY);
      ghost.setAttribute("transform", `translate(${b.x - a.x} ${b.y - a.y})`);
    });
    g.addEventListener("pointerup", (e) => {
      const moved = !!ghost;
      ghost?.remove();
      ghost = null;
      down = null;
      if (!moved) return this.actions.select({ kind: "character", id });
      const to = this.roomAt(this.toMap(e.clientX, e.clientY));
      if (to && to !== room) this.actions.moveCharacter(id, to);
    });
  }

  private roomAt(pt: Point): string | null {
    for (const [id, p] of this.positions) {
      const { w, h } = boxOf(p);
      if (Math.abs(pt.x - p.x) <= w / 2 && Math.abs(pt.y - p.y) <= h / 2) return id;
    }
    return null;
  }

  /**
   * The resize grip on a selected room's bottom-right corner. The box grows
   * both ways from its centre, so the room stays where it is; a
   * double-click puts it back to the usual size.
   */
  private drawGrip(layer: SVGGElement, id: string) {
    const p = this.positions.get(id)!;
    const start = boxOf(p);
    const g = svg("g", { class: "grip" }, layer);
    const at = (s: Size) => ({ x: p.x + s.w / 2, y: p.y + s.h / 2 });
    const c = at(start);
    // Southeast of the box, just inside the SE handle's corner; the SE + handle sits further out.
    svg("rect", { x: c.x - 7, y: c.y - 7, width: 10, height: 10, rx: 2 }, g);
    const title = svg("title", {}, g);
    title.textContent = "Drag to make the room's box bigger or smaller (only on this map). Double-click for the usual size.";
    let down: { x: number; y: number } | null = null;
    let size = start;
    g.addEventListener("pointerdown", (e) => {
      e.stopPropagation();
      g.setPointerCapture(e.pointerId);
      down = { x: e.clientX, y: e.clientY };
    });
    g.addEventListener("pointermove", (e) => {
      if (!down) return;
      const snap = (v: number) => Math.round(v / 10) * 10;
      const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));
      // Growing from the centre: the corner moves half as far as the box grows.
      size = {
        w: clamp(snap(start.w + (2 * (e.clientX - down.x)) / this.scale), MIN_BOX.w, MAX_BOX.w),
        h: clamp(snap(start.h + (2 * (e.clientY - down.y)) / this.scale), MIN_BOX.h, MAX_BOX.h),
      };
      const box = this.root.querySelector(`[data-room="${CSS.escape(id)}"] rect`);
      box?.setAttribute("x", String(-size.w / 2));
      box?.setAttribute("y", String(-size.h / 2));
      box?.setAttribute("width", String(size.w));
      box?.setAttribute("height", String(size.h));
      const r = g.querySelector("rect")!;
      const nc = at(size);
      r.setAttribute("x", String(nc.x - 7));
      r.setAttribute("y", String(nc.y - 7));
    });
    g.addEventListener("pointerup", () => {
      if (!down) return;
      down = null;
      if (size.w !== start.w || size.h !== start.h) this.actions.resizeRoom(id, size);
    });
    g.addEventListener("dblclick", (e) => {
      e.stopPropagation();
      this.actions.resizeRoom(id, null);
    });
  }

  private drawHandles(layer: SVGGElement, id: string) {
    const p = this.positions.get(id);
    if (!p || !this.model) return;
    const { w, h } = boxOf(p);
    this.drawGrip(layer, id);
    const taken = new Set(this.model.exits(id).map((e) => e.dir));
    for (const [dir, [fx, fy]] of Object.entries(HANDLE) as [Direction, [number, number]][]) {
      if (taken.has(dir)) continue;
      const c = { x: p.x + fx * (w / 2 + 14), y: p.y + fy * (h / 2 + 14) };
      const g = svg("g", { class: "handle" }, layer);
      svg("circle", { cx: c.x, cy: c.y, r: 10 }, g);
      const t = svg("text", { x: c.x, y: c.y }, g);
      t.textContent = "+";
      const title = svg("title", {}, g);
      title.textContent = `Add a room to the ${dir}, or drag to a room to connect them`;
      let rubber: SVGLineElement | null = null;
      let over: string | null = null;
      let down = { x: 0, y: 0 };
      g.addEventListener("pointerdown", (e) => {
        e.stopPropagation();
        g.setPointerCapture(e.pointerId);
        down = { x: e.clientX, y: e.clientY };
        rubber = svg("line", { class: "rubber", x1: c.x, y1: c.y, x2: c.x, y2: c.y }, this.root);
      });
      g.addEventListener("pointermove", (e) => {
        if (!rubber) return;
        const m = this.toMap(e.clientX, e.clientY);
        rubber.setAttribute("x2", String(m.x));
        rubber.setAttribute("y2", String(m.y));
        const target = this.roomAt(m);
        if (target !== over) {
          this.root.querySelector(".drop-target")?.classList.remove("drop-target");
          over = target && target !== id ? target : null;
          if (over) this.root.querySelector(`[data-room="${CSS.escape(over)}"]`)?.classList.add("drop-target");
        }
      });
      g.addEventListener("pointerup", (e) => {
        if (!rubber) return;
        const m = this.toMap(e.clientX, e.clientY);
        // A click, not a drag: the pointer barely moved.
        const moved = Math.hypot(e.clientX - down.x, e.clientY - down.y) > 6;
        rubber.remove();
        rubber = null;
        const target = this.roomAt(m);
        if (!moved) this.actions.addRoomBeside(id, dir);
        else if (target && target !== id) this.actions.connect(id, dir, target);
        else if (!target) this.actions.addRoomBeside(id, dir, m);
      });
    }
  }
}
