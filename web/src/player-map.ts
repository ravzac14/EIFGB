// The player's map: the rooms they've been in, drawn as boxes joined by the
// ways between them, with stubs for ways out they haven't taken yet.
// Rooms sit where the author put them in the editor (map.yaml), if the game
// has one; otherwise each is placed by compass direction from a neighbour.
import type { PlayerMap } from "./engine.ts";
import { COLOURS, colourStyle } from "./colours.ts";

type Point = { x: number; y: number };
/** A room's place, and its box's size if the author resized it in the editor. */
type Place = Point & { w?: number; h?: number; colour?: string };

/** Grid spacing and room box size, in SVG units. */
const CELL = { x: 160, y: 90 };
const BOX = { w: 124, h: 44 };
/** The editor's grid spacing, to scale map.yaml positions to ours. */
const EDITOR_CELL = { x: 210, y: 130 };
/** The editor's usual room box, to scale resized boxes to ours. */
const EDITOR_BOX = { w: 150, h: 56 };
const boxOf = (p: Place) => ({ w: p.w ?? BOX.w, h: p.h ?? BOX.h });

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
export function authorPositions(text: string | undefined): Map<string, Place> {
  const out = new Map<string, Place>();
  if (!text) return out;
  // map.yaml is written by the editor: `rooms:` then `  id: [x, y]` lines,
  // or `[x, y, width, height]` for a room whose box was resized.
  for (const m of text.matchAll(/^\s+([\w-]+):\s*\[([-\d.,\s]+)\]/gm)) {
    const n = m[2].split(",").map((v) => Number(v.trim()));
    if ((n.length !== 2 && n.length !== 4) || n.some((v) => !Number.isFinite(v))) continue;
    const p: Place = { x: (n[0] * CELL.x) / EDITOR_CELL.x, y: (n[1] * CELL.y) / EDITOR_CELL.y };
    if (n.length === 4) {
      p.w = (n[2] * BOX.w) / EDITOR_BOX.w;
      p.h = (n[3] * BOX.h) / EDITOR_BOX.h;
    }
    out.set(m[1], p);
  }
  // Room colours: a `colours:` section of `  id: colour` lines.
  const colours = text.match(/^colours:\s*\n((?:[ \t]+.*\n?)*)/m)?.[1] ?? "";
  for (const m of colours.matchAll(/^\s+([\w-]+):\s*([a-z]+)\s*$/gm)) {
    const p = out.get(m[1]);
    if (p && COLOURS[m[2]]) p.colour = m[2];
  }
  return out;
}

/**
 * Positions for every visited room. Author positions win; the rest are
 * placed a step from a placed neighbour (breadth first from the first room),
 * moving to the nearest free spot if that one's taken.
 */
export function placeRooms(map: PlayerMap, author: Map<string, Place>): Map<string, Place> {
  const pos = new Map<string, Place>();
  const used = new Set<string>();
  const cellOf = (p: Point) => `${Math.round(p.x / CELL.x)},${Math.round(p.y / CELL.y)}`;
  const put = (id: string, p: Place) => {
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
function edge(c: Place, d: Point): Point {
  const dx = d.x - c.x, dy = d.y - c.y;
  if (!dx && !dy) return c;
  const b = boxOf(c);
  const t = Math.min(dx ? b.w / 2 / Math.abs(dx) : Infinity, dy ? b.h / 2 / Math.abs(dy) : Infinity);
  return { x: c.x + dx * t, y: c.y + dy * t };
}

/** Splits a room name over at most two lines that fit a box. */
function wrap(name: string, width: number): string[] {
  const max = Math.max(8, Math.floor(width / 7.3));
  if (name.length <= max) return [name];
  const words = name.split(/\s+/);
  let a = "";
  while (words.length && (a + " " + words[0]).trim().length <= max) a = (a + " " + words.shift()).trim();
  if (!a) a = words.shift()!;
  let b = words.join(" ");
  if (b.length > max) b = b.slice(0, max - 1) + "…";
  return b ? [a, b] : [a];
}

/**
 * The map in words, for screen readers: where you are, then each place
 * you've been and its ways out ("Kitchen: north to the Hall; east, not
 * visited yet").
 */
export function mapDescription(map: PlayerMap, names = new Map(map.rooms.map((r) => [r.id, r.name]))): string {
  const places = map.rooms.map((r) => {
    const ways = map.exits
      .filter((e) => e.from === r.id)
      .map((e) => `${e.dir}${e.to ? ` to ${names.get(e.to) ?? e.to}` : ", not visited yet"}${e.closed ? ` (${e.door ?? "door"} closed)` : ""}`);
    return `${r.name}${r.id === map.here ? " (you're here)" : ""}${ways.length ? `: ${ways.join("; ")}` : ""}`;
  });
  return `Map of the ${map.rooms.length === 1 ? "place" : `${map.rooms.length} places`} you've been. ${places.join(". ")}.`;
}

/** Draws the map into `into` (replacing what's there) and scrolls the current room into view. */
export function drawMap(into: HTMLElement, map: PlayerMap, author: Map<string, Place>) {
  const pos = placeRooms(map, author);
  const ps = [...pos.values()];
  const pad = 46;
  const minX = Math.min(...ps.map((p) => p.x - boxOf(p).w / 2)) - pad, minY = Math.min(...ps.map((p) => p.y - boxOf(p).h / 2)) - pad;
  const w = Math.max(...ps.map((p) => p.x + boxOf(p).w / 2)) + pad - minX, h = Math.max(...ps.map((p) => p.y + boxOf(p).h / 2)) + pad - minY;
  const root = svg("svg", { viewBox: `${minX} ${minY} ${w} ${h}`, width: w, height: h, role: "img", class: "pmap" });
  const names = new Map(map.rooms.map((r) => [r.id, r.name]));
  root.setAttribute("aria-label", mapDescription(map, names));
  const here = pos.get(map.here);

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
      lines.append(g);
    }
  }
  root.append(lines);

  const boxes = svg("g", { class: "rooms" });
  for (const [id, p] of pos) {
    const g = svg("g", { class: `room${id === map.here ? " here" : ""}${p.colour ? " coloured" : ""}` });
    if (p.colour) g.setAttribute("style", colourStyle(p.colour));
    const b = boxOf(p);
    g.append(svg("rect", { x: p.x - b.w / 2, y: p.y - b.h / 2, width: b.w, height: b.h, rx: 6 }));
    const parts = wrap(names.get(id) ?? id, b.w);
    parts.forEach((t, i) => g.append(svg("text", { x: p.x, y: p.y + 5 + (i - (parts.length - 1) / 2) * 15 }, t)));
    g.append(svg("title", {}, id === map.here ? `${names.get(id)} (you are here)` : names.get(id) ?? id));
    boxes.append(g);
  }
  root.append(boxes);
  into.replaceChildren(root);
  if (here) {
    into.scrollLeft = here.x - minX - into.clientWidth / 2;
    into.scrollTop = here.y - minY - into.clientHeight / 2;
  }
}
