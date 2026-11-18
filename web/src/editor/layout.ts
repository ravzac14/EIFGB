// Where rooms sit on the editor's map. Positions the author has chosen are
// kept in `map.yaml`, beside game.yaml (the game never reads it); any room
// without one is placed by compass direction from its neighbours.

import { parse, stringify } from "yaml";
import type { Direction, Model } from "./model.ts";

export type Point = { x: number; y: number };
export type Positions = Map<string, Point>;

/** Room boxes are this far apart on the grid. */
export const CELL = { x: 210, y: 130 };
export const MAP_FILE = "map.yaml";

const STEP: Record<Direction, [number, number]> = {
  north: [0, -1], south: [0, 1], east: [1, 0], west: [-1, 0],
  northeast: [1, -1], northwest: [-1, -1], southeast: [1, 1], southwest: [-1, 1],
  up: [1, -1], down: [-1, 1], in: [1, 0], out: [-1, 0],
};

export function readMap(text: string | undefined): Positions {
  const out: Positions = new Map();
  if (!text) return out;
  try {
    const rooms = parse(text)?.rooms ?? {};
    for (const [id, p] of Object.entries(rooms)) {
      if (Array.isArray(p) && p.length === 2 && p.every((n) => typeof n === "number")) out.set(id, { x: p[0], y: p[1] });
    }
  } catch {
    /* a broken map file just means automatic positions */
  }
  return out;
}

export function writeMap(positions: Positions, ids: string[]): string {
  const rooms: Record<string, [number, number]> = {};
  for (const id of ids) {
    const p = positions.get(id);
    if (p) rooms[id] = [Math.round(p.x), Math.round(p.y)];
  }
  const body = stringify({ rooms }, { flowCollectionPadding: false, collectionStyle: "any" }).replace(/^(\s+\w+):\n\s+- (-?\d+)\n\s+- (-?\d+)/gm, "$1: [$2, $3]");
  return `# Where the editor draws each room on its map. The game never reads this file.\n${body}`;
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
