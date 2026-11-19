// The player's map: the rooms they've been in, drawn as boxes joined by the
// ways between them, with stubs for ways out they haven't taken yet.
// Rooms sit where the author put them in the editor (map.yaml), if the game
// has one; otherwise each is placed by compass direction from a neighbour.
import type { PlayerMap } from "./engine.ts";

type Point = { x: number; y: number };

/** Grid spacing and room box size, in SVG units. */
const CELL = { x: 160, y: 90 };
const BOX = { w: 124, h: 44 };
/** The editor's grid spacing, to scale map.yaml positions to ours. */
const EDITOR_CELL = { x: 210, y: 130 };

const STEP: Record<string, [number, number]> = {
  north: [0, -1], south: [0, 1], east: [1, 0], west: [-1, 0],
  northeast: [1, -1], northwest: [-1, -1], southeast: [1, 1], southwest: [-1, 1],
  up: [1, -1], down: [-1, 1], in: [1, 0], out: [-1, 0],
};
/** Directions that don't point across the page get a label on their line. */
const LABEL: Record<string, string> = { up: "up", down: "down", in: "in", out: "out" };

const SVG = "http://www.w3.org/2000/svg";
function svg<K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number> = {}, text?: string) {
  const e = document.createElementNS(SVG, tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
  if (text !== undefined) e.textContent = text;
  return e;
}

/** Author positions from a game's map.yaml, in our units. */
export function authorPositions(text: string | undefined): Map<string, Point> {
  const out = new Map<string, Point>();
  if (!text) return out;
  // map.yaml is written by the editor: `rooms:` then `  id: [x, y]` lines.
  for (const m of text.matchAll(/^\s+([\w-]+):\s*\[\s*(-?\d+(?:\.\d+)?)\s*,\s*(-?\d+(?:\.\d+)?)\s*\]/gm)) {
    out.set(m[1], { x: (Number(m[2]) * CELL.x) / EDITOR_CELL.x, y: (Number(m[3]) * CELL.y) / EDITOR_CELL.y });
  }
  return out;
}

/**
 * Positions for every visited room. Author positions win; the rest are
 * placed a step from a placed neighbour (breadth first from the first room),
 * moving to the nearest free spot if that one's taken.
 */
export function placeRooms(map: PlayerMap, author: Map<string, Point>): Map<string, Point> {
  const pos = new Map<string, Point>();
  const used = new Set<string>();
  const cellOf = (p: Point) => `${Math.round(p.x / CELL.x)},${Math.round(p.y / CELL.y)}`;
  const put = (id: string, p: Point) => {
    pos.set(id, p);
    used.add(cellOf(p));
  };
  for (const r of map.rooms) {
    const p = author.get(r.id);
    if (p) put(r.id, p);
  }
  const free = (x: number, y: number): Point => {
    for (let r = 0; ; r++)
      for (let dx = -r; dx <= r; dx++)
        for (let dy = -r; dy <= r; dy++) {
          if (Math.max(Math.abs(dx), Math.abs(dy)) !== r) continue;
          const p = { x: (x + dx) * CELL.x, y: (y + dy) * CELL.y };
          if (!used.has(cellOf(p))) return p;
        }
  };
  const links = new Map<string, { dir: string; to: string; back: boolean }[]>();
  for (const e of map.exits) {
    if (!e.to) continue;
    (links.get(e.from) ?? links.set(e.from, []).get(e.from)!).push({ dir: e.dir, to: e.to, back: false });
    (links.get(e.to) ?? links.set(e.to, []).get(e.to)!).push({ dir: e.dir, to: e.from, back: true });
  }
  for (const r of map.rooms) {
    if (pos.has(r.id)) continue;
    // A room not joined to anything placed yet starts its own island below.
    const bottom = Math.max(0, ...[...pos.values()].map((p) => Math.round(p.y / CELL.y) + 2));
    if (pos.size) put(r.id, free(0, bottom));
    else put(r.id, { x: 0, y: 0 });
    const queue = [r.id];
    while (queue.length) {
      const id = queue.shift()!;
      const p = pos.get(id)!;
      for (const l of links.get(id) ?? []) {
        if (pos.has(l.to)) continue;
        const [dx, dy] = STEP[l.dir] ?? [1, 0];
        const s = l.back ? -1 : 1;
        put(l.to, free(Math.round(p.x / CELL.x) + dx * s, Math.round(p.y / CELL.y) + dy * s));
        queue.push(l.to);
      }
    }
  }
  return pos;
}

/** Where a line from a box's centre towards `d` leaves the box. */
function edge(c: Point, d: Point): Point {
  const dx = d.x - c.x, dy = d.y - c.y;
  if (!dx && !dy) return c;
  const t = Math.min(dx ? BOX.w / 2 / Math.abs(dx) : Infinity, dy ? BOX.h / 2 / Math.abs(dy) : Infinity);
  return { x: c.x + dx * t, y: c.y + dy * t };
}

/** Splits a room name over at most two lines that fit a box. */
function wrap(name: string): string[] {
  const max = 17;
  if (name.length <= max) return [name];
  const words = name.split(/\s+/);
  let a = "";
  while (words.length && (a + " " + words[0]).trim().length <= max) a = (a + " " + words.shift()).trim();
  if (!a) a = words.shift()!;
  let b = words.join(" ");
  if (b.length > max) b = b.slice(0, max - 1) + "…";
  return b ? [a, b] : [a];
}

export interface DrawOptions {
  /** Called with a direction when the player clicks a neighbouring room or an unexplored way. */
  go?: (dir: string) => void;
}

/** Draws the map into `into` (replacing what's there) and scrolls the current room into view. */
export function drawMap(into: HTMLElement, map: PlayerMap, author: Map<string, Point>, opts: DrawOptions = {}) {
  const pos = placeRooms(map, author);
  const xs = [...pos.values()].map((p) => p.x), ys = [...pos.values()].map((p) => p.y);
  const pad = 46;
  const minX = Math.min(...xs) - BOX.w / 2 - pad, minY = Math.min(...ys) - BOX.h / 2 - pad;
  const w = Math.max(...xs) - Math.min(...xs) + BOX.w + pad * 2, h = Math.max(...ys) - Math.min(...ys) + BOX.h + pad * 2;
  const root = svg("svg", { viewBox: `${minX} ${minY} ${w} ${h}`, width: w, height: h, role: "img", class: "pmap" });
  root.setAttribute("aria-label", `Map of the ${map.rooms.length === 1 ? "place" : `${map.rooms.length} places`} you've been`);
  const names = new Map(map.rooms.map((r) => [r.id, r.name]));
  const here = pos.get(map.here);
  /** Directions from here, by the room they lead to. */
  const fromHere = new Map<string, string>();
  for (const e of map.exits) if (e.from === map.here && e.to && !fromHere.has(e.to)) fromHere.set(e.to, e.dir);

  const lines = svg("g", { class: "ways" });
  const seen = new Set<string>();
  for (const e of map.exits) {
    const a = pos.get(e.from);
    if (!a) continue;
    if (e.to) {
      const b = pos.get(e.to);
      const pair = [e.from, e.to].sort().join(" ");
      if (!b || seen.has(pair)) continue;
      seen.add(pair);
      const p = edge(a, b), q = edge(b, a);
      const g = svg("g");
      g.append(svg("line", { x1: p.x, y1: p.y, x2: q.x, y2: q.y }));
      const mid = { x: (p.x + q.x) / 2, y: (p.y + q.y) / 2 };
      if (e.door) {
        // A door: a bar across the way, filled when it's shut.
        const ang = (Math.atan2(q.y - p.y, q.x - p.x) * 180) / Math.PI;
        g.append(svg("rect", { x: mid.x - 3, y: mid.y - 9, width: 6, height: 18, class: e.closed ? "door shut" : "door", transform: `rotate(${ang} ${mid.x} ${mid.y})` }));
        g.append(svg("title", {}, `${e.door} (${e.closed ? "closed" : "open"})`));
      }
      // Up, down, in and out are labelled beside the line, clear of any door.
      if (LABEL[e.dir]) {
        const len = Math.hypot(q.x - p.x, q.y - p.y) || 1;
        const n = { x: -(q.y - p.y) / len, y: (q.x - p.x) / len };
        if (n.y > 0) (n.x = -n.x), (n.y = -n.y);
        g.append(svg("text", { x: mid.x + n.x * 16, y: mid.y + n.y * 16 + 4, class: "dir" }, LABEL[e.dir]));
      }
      lines.append(g);
    } else {
      // A way out that hasn't been taken: a short dashed stub ending in "?".
      const [dx, dy] = STEP[e.dir] ?? [1, 0];
      const len = Math.hypot(dx * CELL.x, dy * CELL.y);
      const far = { x: a.x + (dx * CELL.x) / len * 200, y: a.y + (dy * CELL.y) / len * 200 };
      const p = edge(a, far);
      const end = { x: p.x + (dx * CELL.x) / len * 26, y: p.y + (dy * CELL.y) / len * 26 };
      const g = svg("g", { class: "stub" });
      g.append(svg("line", { x1: p.x, y1: p.y, x2: end.x, y2: end.y }), svg("text", { x: end.x + (dx * 8), y: end.y + dy * 8 + 4 }, "?"));
      g.append(svg("title", {}, `${e.dir}${e.door ? `, through the ${e.door}` : ""}: not explored yet`));
      if (LABEL[e.dir]) g.append(svg("text", { x: (p.x + end.x) / 2, y: (p.y + end.y) / 2 - 6, class: "dir" }, LABEL[e.dir]));
      if (e.from === map.here && opts.go) clickable(g, `Go ${e.dir}`, () => opts.go!(e.dir));
      lines.append(g);
    }
  }
  root.append(lines);

  const boxes = svg("g", { class: "rooms" });
  for (const [id, p] of pos) {
    const g = svg("g", { class: id === map.here ? "room here" : "room" });
    g.append(svg("rect", { x: p.x - BOX.w / 2, y: p.y - BOX.h / 2, width: BOX.w, height: BOX.h, rx: 6 }));
    const parts = wrap(names.get(id) ?? id);
    parts.forEach((t, i) => g.append(svg("text", { x: p.x, y: p.y + 5 + (i - (parts.length - 1) / 2) * 15 }, t)));
    g.append(svg("title", {}, id === map.here ? `${names.get(id)} (you are here)` : names.get(id) ?? id));
    const dir = fromHere.get(id);
    if (dir && opts.go) clickable(g, `Go ${dir}, to ${names.get(id)}`, () => opts.go!(dir));
    boxes.append(g);
  }
  root.append(boxes);
  into.replaceChildren(root);
  if (here) {
    into.scrollLeft = here.x - minX - into.clientWidth / 2;
    into.scrollTop = here.y - minY - into.clientHeight / 2;
  }
}

function clickable(g: SVGGElement, label: string, go: () => void) {
  g.classList.add("go");
  g.setAttribute("role", "button");
  g.setAttribute("tabindex", "0");
  g.setAttribute("aria-label", label);
  g.addEventListener("click", go);
  g.addEventListener("keydown", (ev) => {
    if (ev.key === "Enter" || ev.key === " ") {
      ev.preventDefault();
      go();
    }
  });
}
