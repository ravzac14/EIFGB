// A conversation as a graph, on the map's canvas while it's selected: each
// part a box with what's said and the replies, a line from each reply to
// the part it leads to (or to "End"). Drag a part to arrange it; drag from
// a reply's dot onto a part to make it lead there, or into empty space for
// a new part. Where the parts are is kept in map.yaml, beside the rooms.

import type { Choice, ChoiceNode } from "./model.ts";
import type { Point } from "./layout.ts";

export type TalkPositions = Map<string, Map<string, Point>>;

export interface TalkActions {
  /** A part was clicked: show it in the details panel. */
  pick(node: string): void;
  move(node: string, to: Point): void;
  /** A reply now leads to `to` (a part), or to a new part placed at `at`. */
  link(node: string, option: number, to: { node: string } | { at: Point }): void;
}

const W = 230;
const HEAD = 46;
const ROW = 22;
const GAP = { x: 90, y: 30 };

const heightOf = (n: ChoiceNode) => HEAD + Math.max(1, n.options.length) * ROW + 8;

function svg<K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number> = {}, parent?: Element) {
  const e = document.createElementNS("http://www.w3.org/2000/svg", tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
  parent?.append(e);
  return e;
}

const clip = (s: string, n: number) => (s.length > n ? `${s.slice(0, n - 1).trimEnd()}…` : s);

/** Places for the parts that have none: by how many replies from the start, in columns. */
export function talkLayout(c: Choice, saved: Map<string, Point> | undefined): Map<string, Point> {
  const out = new Map<string, Point>();
  for (const n of c.nodes) {
    const p = saved?.get(n.id);
    if (p) out.set(n.id, p);
  }
  const depth = new Map<string, number>();
  const start = c.nodes.find((n) => n.id === c.start) ?? c.nodes[0];
  if (!start) return out;
  const queue = [start.id];
  depth.set(start.id, 0);
  while (queue.length) {
    const id = queue.shift()!;
    const n = c.nodes.find((x) => x.id === id);
    if (!n) continue;
    for (const to of [...n.options.map((o) => o.goto), n.goto].filter(Boolean)) {
      if (!depth.has(to) && c.nodes.some((x) => x.id === to)) {
        depth.set(to, depth.get(id)! + 1);
        queue.push(to);
      }
    }
  }
  // Parts nothing leads to: a column of their own after the rest.
  const last = Math.max(0, ...depth.values()) + 1;
  const columnY = new Map<number, number>();
  for (const n of c.nodes) {
    if (out.has(n.id)) continue;
    const d = depth.get(n.id) ?? last;
    const y = columnY.get(d) ?? 0;
    out.set(n.id, { x: d * (W + GAP.x), y });
    columnY.set(d, y + heightOf(n) + GAP.y);
  }
  return out;
}

export class TalkView {
  private root: SVGGElement;
  private pan = { x: 40, y: 40 };
  private places = new Map<string, Point>();
  private choice: Choice | null = null;

  private svgEl: SVGSVGElement;
  private actions: TalkActions;

  constructor(svgEl: SVGSVGElement, actions: TalkActions) {
    this.svgEl = svgEl;
    this.actions = actions;
    const marker = svg("marker", { id: "talk-arrow", viewBox: "0 0 10 10", refX: 9, refY: 5, markerWidth: 7, markerHeight: 7, orient: "auto-start-reverse" }, svg("defs", {}, svgEl));
    svg("path", { d: "M 0 0 L 10 5 L 0 10 z" }, marker);
    this.root = svg("g", {}, svgEl);
    // Drag the background to look around.
    svgEl.addEventListener("pointerdown", (e) => {
      if (e.target !== svgEl) return;
      const from = { x: e.clientX - this.pan.x, y: e.clientY - this.pan.y };
      const move = (m: PointerEvent) => {
        this.pan = { x: m.clientX - from.x, y: m.clientY - from.y };
        this.apply();
      };
      const up = () => {
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", up);
      };
      window.addEventListener("pointermove", move);
      window.addEventListener("pointerup", up);
    });
  }

  private apply() {
    this.root.setAttribute("transform", `translate(${this.pan.x} ${this.pan.y})`);
  }

  private toGraph(clientX: number, clientY: number): Point {
    const r = this.svgEl.getBoundingClientRect();
    return { x: clientX - r.left - this.pan.x, y: clientY - r.top - this.pan.y };
  }

  /** The part under a point, if any. */
  private nodeAt(p: Point): string | null {
    if (!this.choice) return null;
    for (const n of this.choice.nodes) {
      const at = this.places.get(n.id);
      if (at && p.x >= at.x && p.x <= at.x + W && p.y >= at.y && p.y <= at.y + heightOf(n)) return n.id;
    }
    return null;
  }

  render(c: Choice, places: Map<string, Point>, picked: string | null) {
    const fresh = this.choice?.id !== c.id;
    this.choice = c;
    this.places = places;
    this.root.replaceChildren();
    if (fresh) this.pan = { x: 40, y: 40 };
    this.apply();
    const lines = svg("g", { class: "talk-lines" }, this.root);
    const boxes = svg("g", {}, this.root);
    const ends: Point[] = [];
    for (const n of c.nodes) {
      const at = places.get(n.id)!;
      const g = svg("g", { class: `talk-node${n.id === c.start ? " start" : ""}${n.id === picked ? " picked" : ""}`, "data-node": n.id, transform: `translate(${at.x} ${at.y})` }, boxes);
      svg("rect", { width: W, height: heightOf(n), rx: 8 }, g);
      const head = svg("text", { x: 10, y: 18, class: "talk-name" }, g);
      head.textContent = `${n.id === c.start ? "▶ " : ""}${n.id.replace(/_/g, " ")}`;
      const said = svg("text", { x: 10, y: 36, class: "talk-said" }, g);
      said.textContent = clip(n.text.replace(/\s+/g, " ") || "(nothing said)", 34);
      const title = svg("title", {}, g);
      title.textContent = n.text || n.id;
      n.options.forEach((o, i) => {
        const y = HEAD + i * ROW + ROW / 2;
        const t = svg("text", { x: 14, y: y + 4, class: "talk-option" }, g);
        t.textContent = clip(o.text || "(a reply)", 30);
        const dot = svg("circle", { cx: W, cy: y, r: 6, class: "talk-port" }, g);
        const tt = svg("title", {}, dot);
        tt.textContent = "Drag onto a part to lead there, or into empty space for a new part";
        this.dragLink(dot, n.id, i, { x: at.x + W, y: at.y + y });
        // The line to where it leads.
        const from = { x: at.x + W, y: at.y + y };
        if (o.end) ends.push(from);
        else if (o.goto && places.has(o.goto)) this.line(lines, from, o.goto, c);
      });
      if (!n.options.length) {
        const t = svg("text", { x: 14, y: HEAD + 14, class: "talk-option dim" }, g);
        t.textContent = n.goto ? "then goes on" : "then ends";
        const from = { x: at.x + W, y: at.y + HEAD + ROW / 2 };
        if (n.goto && places.has(n.goto)) this.line(lines, from, n.goto, c);
        else ends.push(from);
      }
      this.dragNode(g, n.id, at);
    }
    // One "End" for all the replies that end it, right of everything.
    if (ends.length) {
      const right = Math.max(...[...places.values()].map((p) => p.x)) + W + GAP.x;
      const mid = ends.reduce((s, p) => s + p.y, 0) / ends.length;
      const endAt = { x: right, y: mid };
      const g = svg("g", { class: "talk-end", transform: `translate(${endAt.x} ${endAt.y - 14})` }, boxes);
      svg("rect", { width: 60, height: 28, rx: 14 }, g);
      const t = svg("text", { x: 30, y: 18 }, g);
      t.textContent = "End";
      for (const f of ends) svg("path", { d: curve(f, { x: endAt.x, y: endAt.y }), "marker-end": "url(#talk-arrow)", class: "to-end" }, lines);
    }
  }

  private line(layer: Element, from: Point, to: string, c: Choice) {
    const at = this.places.get(to)!;
    const n = c.nodes.find((x) => x.id === to)!;
    // Into the part's left side; back to an earlier part, around its top.
    const end = { x: at.x, y: at.y + Math.min(heightOf(n) / 2, 24) };
    svg("path", { d: curve(from, end), "marker-end": "url(#talk-arrow)" }, layer);
  }

  private dragNode(g: SVGGElement, id: string, at: Point) {
    g.addEventListener("pointerdown", (e) => {
      if ((e.target as Element).classList.contains("talk-port")) return;
      e.stopPropagation();
      const start = { x: e.clientX, y: e.clientY };
      let moved = false;
      const move = (m: PointerEvent) => {
        const dx = m.clientX - start.x;
        const dy = m.clientY - start.y;
        if (!moved && Math.hypot(dx, dy) < 4) return;
        moved = true;
        g.setAttribute("transform", `translate(${at.x + dx} ${at.y + dy})`);
      };
      const up = (u: PointerEvent) => {
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", up);
        if (moved) this.actions.move(id, { x: Math.round((at.x + u.clientX - start.x) / 10) * 10, y: Math.round((at.y + u.clientY - start.y) / 10) * 10 });
        else this.actions.pick(id);
      };
      window.addEventListener("pointermove", move);
      window.addEventListener("pointerup", up);
    });
  }

  private dragLink(dot: SVGCircleElement, node: string, option: number, from: Point) {
    dot.addEventListener("pointerdown", (e) => {
      e.stopPropagation();
      const rubber = svg("path", { class: "talk-rubber" }, this.root);
      const move = (m: PointerEvent) => rubber.setAttribute("d", curve(from, this.toGraph(m.clientX, m.clientY)));
      const up = (u: PointerEvent) => {
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", up);
        rubber.remove();
        const p = this.toGraph(u.clientX, u.clientY);
        if (Math.hypot(p.x - from.x, p.y - from.y) < 12) return;
        const target = this.nodeAt(p);
        this.actions.link(node, option, target ? { node: target } : { at: { x: Math.round(p.x / 10) * 10, y: Math.round(p.y / 10) * 10 } });
      };
      window.addEventListener("pointermove", move);
      window.addEventListener("pointerup", up);
    });
  }
}

/** A smooth line from one point to another, leaving rightwards. */
function curve(a: Point, b: Point): string {
  const dx = Math.max(40, Math.abs(b.x - a.x) / 2);
  return `M ${a.x} ${a.y} C ${a.x + dx} ${a.y}, ${b.x - dx} ${b.y}, ${b.x} ${b.y}`;
}
