// Map tidy-up: laying rooms out again by their directions, and spotting
// ways between rooms that are hard to read on the map: lines that cross,
// lines that run through another room, and exits drawn pointing the
// opposite way to their direction.

import { boxOf, layout, type Point, type Positions } from "./layout.ts";
import type { Direction, Model } from "./model.ts";

/** Every room placed again by its directions from the start room, keeping its size and colour. */
export function tidy(model: Model, positions: Positions): Positions {
  const fresh = layout(model, new Map());
  for (const [id, p] of fresh) {
    const old = positions.get(id);
    if (!old) continue;
    fresh.set(id, { ...p, ...(old.w ? { w: old.w } : {}), ...(old.h ? { h: old.h } : {}), ...(old.colour ? { colour: old.colour } : {}) });
  }
  return fresh;
}

/** A way between two rooms on the map, either way: its first exit, for selecting it. */
export interface Way {
  room: string;
  dir: Direction;
  to: string;
}

export type MapProblem =
  | { kind: "cross"; a: Way; b: Way }
  | { kind: "through"; a: Way; room: string }
  | { kind: "backwards"; a: Way };

/** Compass directions as screen directions; up, down, in and out can be drawn any way. */
const COMPASS: Partial<Record<Direction, [number, number]>> = {
  north: [0, -1], south: [0, 1], east: [1, 0], west: [-1, 0],
  northeast: [1, -1], northwest: [-1, -1], southeast: [1, 1], southwest: [-1, 1],
};

/** The ways on the map: one per pair of rooms joined, whichever way. */
function ways(model: Model): Way[] {
  const rooms = new Set(model.roomIds());
  const seen = new Set<string>();
  const out: Way[] = [];
  for (const r of model.roomIds()) {
    for (const e of model.exits(r)) {
      if (!e.to || e.to === r || !rooms.has(e.to)) continue;
      const k = [r, e.to].sort().join("|");
      if (seen.has(k)) continue;
      seen.add(k);
      out.push({ room: r, dir: e.dir, to: e.to });
    }
  }
  return out;
}

const cross = (o: Point, a: Point, b: Point) => (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);

/** Whether segments p1–p2 and q1–q2 cross, not counting touching ends. */
function segmentsCross(p1: Point, p2: Point, q1: Point, q2: Point): boolean {
  const d1 = cross(q1, q2, p1);
  const d2 = cross(q1, q2, p2);
  const d3 = cross(p1, p2, q1);
  const d4 = cross(p1, p2, q2);
  if (((d1 > 0 && d2 < 0) || (d1 < 0 && d2 > 0)) && ((d3 > 0 && d4 < 0) || (d3 < 0 && d4 > 0))) return true;
  // On one line, overlapping: one runs along the other.
  if (d1 === 0 && d2 === 0) {
    const along = (a: Point, b: Point, p: Point) => Math.min(a.x, b.x) < p.x && p.x < Math.max(a.x, b.x) || Math.min(a.y, b.y) < p.y && p.y < Math.max(a.y, b.y);
    return along(p1, p2, q1) || along(p1, p2, q2) || along(q1, q2, p1) || along(q1, q2, p2);
  }
  return false;
}

/** Whether the segment a–b passes through the box around `c`. */
function throughBox(a: Point, b: Point, c: Point, size: { w: number; h: number }): boolean {
  const [hw, hh] = [size.w / 2, size.h / 2];
  // Liang–Barsky clipping against the box.
  let t0 = 0;
  let t1 = 1;
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  for (const [p, q] of [[-dx, a.x - (c.x - hw)], [dx, c.x + hw - a.x], [-dy, a.y - (c.y - hh)], [dy, c.y + hh - a.y]]) {
    if (p === 0) {
      if (q < 0) return false;
      continue;
    }
    const t = q / p;
    if (p < 0) t0 = Math.max(t0, t);
    else t1 = Math.min(t1, t);
    if (t0 > t1) return false;
  }
  return t1 - t0 > 0.02;
}

/** What's hard to read on the map as it's laid out. */
export function mapProblems(model: Model, positions: Positions): MapProblem[] {
  const out: MapProblem[] = [];
  const ws = ways(model).filter((w) => positions.has(w.room) && positions.has(w.to));
  const at = (id: string) => positions.get(id)!;
  for (let i = 0; i < ws.length; i++) {
    for (let j = i + 1; j < ws.length; j++) {
      const [a, b] = [ws[i], ws[j]];
      // Lines meeting at a room they share don't cross.
      if (a.room === b.room || a.room === b.to || a.to === b.room || a.to === b.to) continue;
      if (segmentsCross(at(a.room), at(a.to), at(b.room), at(b.to))) out.push({ kind: "cross", a, b });
    }
  }
  for (const w of ws) {
    for (const r of model.roomIds()) {
      if (r === w.room || r === w.to || !positions.has(r)) continue;
      if (throughBox(at(w.room), at(w.to), at(r), boxOf(at(r)))) {
        out.push({ kind: "through", a: w, room: r });
        break;
      }
    }
  }
  for (const r of model.roomIds()) {
    for (const e of model.exits(r)) {
      const step = COMPASS[e.dir];
      if (!step || !e.to || e.to === r || !positions.has(r) || !positions.has(e.to)) continue;
      const [from, to] = [at(r), at(e.to)];
      // More than a right angle away from where it says it goes.
      if ((to.x - from.x) * step[0] + (to.y - from.y) * step[1] < 0) out.push({ kind: "backwards", a: { room: r, dir: e.dir, to: e.to } });
    }
  }
  return out;
}

/** The pair of rooms a way joins, as the map's links are keyed. */
export const wayKey = (w: Way) => [w.room, w.to].sort().join("|");
