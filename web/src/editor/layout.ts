// Where rooms sit on the editor's map. Positions the author has chosen are
// kept in `map.yaml`, beside game.yaml (the game never reads it); any room
// without one is placed by compass direction from its neighbours.

import { parse, stringify } from "yaml";
import type { Direction, Model } from "./model.ts";
import { COLOURS } from "../colours.ts";
export { COLOURS, colourStyle } from "../colours.ts";

export type Point = { x: number; y: number };
/** A room's place on the map, its box's size if the author resized it, and its colour if it has one. */
export type Place = Point & { w?: number; h?: number; colour?: string };

export type Positions = Map<string, Place>;

/** Room boxes are this far apart on the grid. */
export const CELL = { x: 210, y: 130 };
export const MAP_FILE = "map.yaml";
/** A room box's usual size, and the limits when resizing it. */
export const BOX = { w: 150, h: 56 };
export const MIN_BOX = { w: 100, h: 44 };
export const MAX_BOX = { w: 600, h: 400 };

/** A room's box size: its own, or the usual one. */
export function boxOf(p: Place | undefined): { w: number; h: number } {
  return { w: p?.w ?? BOX.w, h: p?.h ?? BOX.h };
}

const STEP: Record<Direction, [number, number]> = {
  north: [0, -1], south: [0, 1], east: [1, 0], west: [-1, 0],
  northeast: [1, -1], northwest: [-1, -1], southeast: [1, 1], southwest: [-1, 1],
  up: [1, -1], down: [-1, 1], in: [1, 0], out: [-1, 0],
};

export function readMap(text: string | undefined): Positions {
  const out: Positions = new Map();
  if (!text) return out;
  try {
    const doc = parse(text) ?? {};
    const rooms = doc.rooms ?? {};
    for (const [id, p] of Object.entries(rooms)) {
      if (!Array.isArray(p) || !p.every((n) => typeof n === "number")) continue;
      // [x, y], or [x, y, width, height] for a resized room.
      if (p.length === 2) out.set(id, { x: p[0], y: p[1] });
      else if (p.length === 4) out.set(id, { x: p[0], y: p[1], w: p[2], h: p[3] });
    }
    // Colours, by room: `colours: { kitchen: green }`.
    for (const [id, c] of Object.entries(doc.colours ?? {})) {
      const p = out.get(id);
      if (p && typeof c === "string" && COLOURS[c]) p.colour = c;
    }
  } catch {
    /* a broken map file just means automatic positions */
  }
  return out;
}

/** Where the parts of each conversation sit in its graph: `talk: { keeper_talk: { start: [0, 0] } }`. */
export function readTalk(text: string | undefined): Map<string, Map<string, Point>> {
  const out = new Map<string, Map<string, Point>>();
  if (!text) return out;
  try {
    const talk = parse(text)?.talk ?? {};
    for (const [choice, nodes] of Object.entries(talk)) {
      const m = new Map<string, Point>();
      for (const [node, p] of Object.entries(nodes as Record<string, unknown>)) {
        if (Array.isArray(p) && p.length === 2 && p.every((n) => typeof n === "number")) m.set(node, { x: p[0], y: p[1] });
      }
      if (m.size) out.set(choice, m);
    }
  } catch {
    /* automatic places */
  }
  return out;
}

export function writeMap(positions: Positions, ids: string[], talk?: Map<string, Map<string, Point>>, choices?: Record<string, string[]>): string {
  const rooms: Record<string, number[]> = {};
  for (const id of ids) {
    const p = positions.get(id);
    if (!p) continue;
    const at = [Math.round(p.x), Math.round(p.y)];
    const sized = (p.w !== undefined && p.w !== BOX.w) || (p.h !== undefined && p.h !== BOX.h);
    rooms[id] = sized ? [...at, Math.round(p.w ?? BOX.w), Math.round(p.h ?? BOX.h)] : at;
  }
  const body = stringify({ rooms }, { flowCollectionPadding: false, collectionStyle: "any" }).replace(
    /^(\s+[\w-]+):\n((?:\s+- -?\d+\n?)+)/gm,
    (_, k: string, list: string) => `${k}: [${[...list.matchAll(/-?\d+/g)].map((m) => m[0]).join(", ")}]\n`,
  );
  const colours = ids.filter((id) => positions.get(id)?.colour).map((id) => `  ${id}: ${positions.get(id)!.colour}`);
  // Conversations: only parts that still exist, of conversations that do.
  const talks: string[] = [];
  for (const [choice, nodes] of talk ?? []) {
    const live = choices?.[choice];
    if (!live) continue;
    const rows = [...nodes].filter(([n]) => live.includes(n)).map(([n, p]) => `    ${n}: [${Math.round(p.x)}, ${Math.round(p.y)}]`);
    if (rows.length) talks.push(`  ${choice}:`, ...rows);
  }
  return `# Where the editor draws each room on its map, the size of any room made\n# bigger or smaller, room colours, and where the parts of each conversation\n# sit in its graph. The game never reads this file.\n${body.replace(/\n+$/, "\n")}${colours.length ? `colours:\n${colours.join("\n")}\n` : ""}${talks.length ? `talk:\n${talks.join("\n")}\n` : ""}`;
}

const key = (x: number, y: number) => `${x},${y}`;

/** The nearest free grid cell to (x, y), spiralling out. */
function freeCell(x: number, y: number, used: Set<string>): [number, number] {
  if (!used.has(key(x, y))) return [x, y];
  for (let r = 1; ; r++) {
    for (let dx = -r; dx <= r; dx++)
      for (let dy = -r; dy <= r; dy++) {
        if (Math.max(Math.abs(dx), Math.abs(dy)) !== r) continue;
        if (!used.has(key(x + dx, y + dy))) return [x + dx, y + dy];
      }
  }
}

/** Fills in positions for rooms that have none, keeping the rest. */
export function layout(model: Model, positions: Positions): Positions {
  const out: Positions = new Map();
  const ids = model.roomIds();
  const used = new Set<string>();
  for (const id of ids) {
    const p = positions.get(id);
    if (p) {
      out.set(id, p);
      used.add(key(Math.round(p.x / CELL.x), Math.round(p.y / CELL.y)));
    }
  }
  const place = (id: string, cx: number, cy: number) => {
    const [x, y] = freeCell(cx, cy, used);
    used.add(key(x, y));
    out.set(id, { x: x * CELL.x, y: y * CELL.y });
  };
  // Breadth-first from the start room, then from any room still unplaced.
  const start = model.start();
  const order = [...(start && ids.includes(start) ? [start] : []), ...ids];
  for (const seed of order) {
    if (!out.has(seed)) place(seed, 0, 0);
    const queue = [seed];
    while (queue.length) {
      const id = queue.shift()!;
      const at = out.get(id)!;
      const cx = Math.round(at.x / CELL.x);
      const cy = Math.round(at.y / CELL.y);
      for (const e of model.exits(id)) {
        if (!e.to || out.has(e.to) || !ids.includes(e.to)) continue;
        const [dx, dy] = STEP[e.dir];
        place(e.to, cx + dx, cy + dy);
        queue.push(e.to);
      }
    }
  }
  return out;
}

/** The compass direction from one point to another, for exits drawn by dragging. */
export function directionBetween(from: Point, to: Point): Direction {
  const angle = (Math.atan2(to.y - from.y, to.x - from.x) * 180) / Math.PI; // 0 = east, 90 = south
  const dirs: Direction[] = ["east", "southeast", "south", "southwest", "west", "northwest", "north", "northeast"];
  return dirs[(Math.round(angle / 45) + 8) % 8];
}

/** Where a new room goes when added `dir` of a room at `from`. */
export function besideRoom(from: Point, dir: Direction, positions: Positions): Point {
  const used = new Set([...positions.values()].map((p) => key(Math.round(p.x / CELL.x), Math.round(p.y / CELL.y))));
  const [dx, dy] = STEP[dir];
  const [x, y] = freeCell(Math.round(from.x / CELL.x) + dx, Math.round(from.y / CELL.y) + dy, used);
  return { x: x * CELL.x, y: y * CELL.y };
}
