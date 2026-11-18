// The editor's view of a game: game.yaml as a YAML document that keeps the
// author's comments and formatting. Every edit the editor makes goes
// through here, as a change to that document, so the text stays the source
// of truth and hand-written files round-trip.

import { Document, isMap, isScalar, isSeq, parseDocument, Scalar, YAMLMap, YAMLSeq, type Pair } from "yaml";

export const DIRECTIONS = [
  "north", "northeast", "east", "southeast", "south", "southwest", "west", "northwest", "up", "down", "in", "out",
] as const;
export type Direction = (typeof DIRECTIONS)[number];
export const OPPOSITE: Record<Direction, Direction> = {
  north: "south", south: "north", east: "west", west: "east",
  northeast: "southwest", southwest: "northeast", northwest: "southeast", southeast: "northwest",
  up: "down", down: "up", in: "out", out: "in",
};
export const isDirection = (s: string): s is Direction => (DIRECTIONS as readonly string[]).includes(s);

/** One exit as the editor understands it. */
export interface Exit {
  dir: Direction;
  /** The room it leads to. */
  to?: string;
  /** The door it goes through. */
  door?: string;
  /** For a blocked exit: what the player is told. */
  blocked?: string;
  /**
   * `plain` (a room), `door`, `blocked` (a message), or `advanced`
   * (conditions, or several exits one way): shown, but edited as text.
   */
  kind: "plain" | "door" | "blocked" | "advanced";
}

export interface Room {
  id: string;
  name: string;
  description: string;
  dark: boolean;
  exits: Exit[];
}

export interface Door {
  id: string;
  name: string;
  /** The room it's placed in. */
  in: string | null;
  open: boolean;
  locked: boolean;
  lockable: boolean;
  key: string | null;
  hidden: boolean;
  description: string;
}

const str = (v: unknown): string | undefined => (typeof v === "string" ? v : isScalar(v) && typeof v.value === "string" ? v.value : undefined);

/** A readable ID from a name: "Dark Cellar" → "dark_cellar". */
export function slug(name: string, fallback = "room"): string {
  let s = name
    .normalize("NFKD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
  if (!/^[a-z]/.test(s)) s = s ? `${fallback}_${s}` : fallback;
  return s;
}

export const NEW_GAME = `format: eifgb/0.1

game:
  id: my_game
  title: "My Game"
  version: "1.0"
  start: first_room
  intro: Your story begins here.

rooms:
  first_room:
    name: First Room
    description: A plain room. Describe what the player sees here.
`;

const OUTPUT = { lineWidth: 0, flowCollectionPadding: false } as const;

/**
 * Carries an edit over to the author's own text. `base` is how the YAML
 * library prints the file as loaded and `next` how it prints it now; lines
 * the edit didn't touch are taken from `original`, exactly as written. The
 * library only rewrites spacing within lines, so `base` and `original` line
 * up one to one; if they ever don't, `next` is used as it is.
 */
export function carryOver(original: string, base: string, next: string): string {
  if (next === base) return original;
  const o = original.split("\n");
  const a = base.split("\n");
  const b = next.split("\n");
  if (o.length !== a.length) return next;
  // Longest common subsequence of a and b, from the end.
  const n = a.length;
  const m = b.length;
  const w = m + 1;
  const lcs = new Uint32Array((n + 1) * w);
  for (let i = n - 1; i >= 0; i--)
    for (let j = m - 1; j >= 0; j--)
      lcs[i * w + j] = a[i] === b[j] ? lcs[(i + 1) * w + j + 1] + 1 : Math.max(lcs[(i + 1) * w + j], lcs[i * w + j + 1]);
  const out: string[] = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (a[i] === b[j]) {
      out.push(o[i]);
      i++;
      j++;
    } else if (lcs[(i + 1) * w + j] >= lcs[i * w + j + 1]) i++;
    else out.push(b[j++]);
  }
  while (j < m) out.push(b[j++]);
  return out.join("\n");
}

export class Model {
  doc: Document;
  private original: string;
  private base: string;

  constructor(text: string) {
    const doc = parseDocument(text);
    if (doc.errors.length) throw new Error(doc.errors[0].message);
    this.doc = doc;
    this.original = text;
    this.base = doc.toString(OUTPUT);
  }

  /** The file, with the author's own text wherever an edit didn't reach. */
  toString(): string {
    return carryOver(this.original, this.base, this.doc.toString(OUTPUT));
  }

  // ---------------------------------------------------------- reading

  private map(path: (string | number)[]): YAMLMap | null {
    const n = this.doc.getIn(path, true);
    return isMap(n) ? n : null;
  }

  private keys(section: string): string[] {
    const m = this.map([section]);
    return m ? m.items.map((p) => String(isScalar(p.key) ? p.key.value : p.key)) : [];
  }

  roomIds(): string[] {
    return this.keys("rooms");
  }

  /** Every ID in the shared namespace (rooms, objects, characters). */
  allIds(): Set<string> {
    return new Set([...this.keys("rooms"), ...this.keys("objects"), ...this.keys("characters")]);
  }

  get(path: (string | number)[]): unknown {
    const v = this.doc.getIn(path);
    return isScalar(v) ? v.value : v;
  }

  title(): string {
    return str(this.get(["game", "title"])) ?? "Untitled game";
  }

  start(): string | undefined {
    return str(this.get(["game", "start"]));
  }

  room(id: string): Room | null {
    const m = this.map(["rooms", id]);
    if (!m) return null;
    return {
      id,
      name: str(m.get("name")) ?? id,
      description: str(m.get("description")) ?? "",
      dark: m.get("dark") === true,
      exits: this.exits(id),
    };
  }

  rooms(): Room[] {
    return this.roomIds()
      .map((id) => this.room(id))
      .filter((r): r is Room => r !== null);
  }

  exits(id: string): Exit[] {
    const m = this.map(["rooms", id, "exits"]);
    if (!m) return [];
    const rooms = new Set(this.roomIds());
    const out: Exit[] = [];
    for (const p of m.items) {
      const dir = String(isScalar(p.key) ? p.key.value : p.key);
      if (!isDirection(dir)) continue;
      const v = p.value;
      const s = str(v);
      if (s !== undefined) {
        out.push(rooms.has(s) ? { dir, to: s, kind: "plain" } : { dir, blocked: s, kind: "blocked" });
      } else if (isMap(v)) {
        const to = str(v.get("to"));
        const through = str(v.get("through"));
        const blocked = str(v.get("blocked"));
        const simple = v.items.every((i) => ["to", "through"].includes(String(isScalar(i.key) ? i.key.value : i.key)));
        if (simple && to) out.push({ dir, to, door: through, kind: through ? "door" : "plain" });
        else out.push({ dir, to, door: through, blocked, kind: "advanced" });
      } else if (isSeq(v)) {
        // Several exits one way: show the first that leads somewhere.
        const first = v.items.map((i) => (isMap(i) ? str(i.get("to")) : str(i))).find((t) => t && rooms.has(t));
        const door = v.items.map((i) => (isMap(i) ? str(i.get("through")) : undefined)).find(Boolean);
        out.push({ dir, to: first, door, kind: "advanced" });
      }
    }
    return out;
  }

  exit(room: string, dir: Direction): Exit | undefined {
    return this.exits(room).find((e) => e.dir === dir);
  }

  /** Objects with the `door` trait. */
  doors(): Door[] {
    return this.keys("objects")
      .map((id) => this.door(id))
      .filter((d): d is Door => d !== null);
  }

  door(id: string): Door | null {
    const m = this.map(["objects", id]);
    if (!m || !this.traits(["objects", id]).includes("door")) return null;
    const state = (k: string) => this.get(["objects", id, "state", k]);
    return {
      id,
      name: str(m.get("name")) ?? id,
      in: str(m.get("in")) ?? null,
      open: state("open") === true,
      locked: state("locked") === true,
      lockable: this.traits(["objects", id]).includes("lockable"),
      key: str(m.get("unlocks_with")) ?? null,
      hidden: state("hidden") === true,
      description: str(m.get("description")) ?? "",
    };
  }

  /** Objects (not doors) that could be keys: portable things. */
  keysAvailable(): { id: string; name: string }[] {
    return this.keys("objects")
      .filter((id) => this.traits(["objects", id]).includes("portable"))
      .map((id) => ({ id, name: str(this.get(["objects", id, "name"])) ?? id }));
  }

  traits(path: string[]): string[] {
    const t = this.doc.getIn([...path, "traits"], true);
    return isSeq(t) ? t.items.map((i) => String(isScalar(i) ? i.value : i)) : [];
  }

  /** What's placed in a room (objects and characters), for warnings when deleting it. */
  thingsIn(room: string): string[] {
    return ["objects", "characters"].flatMap((s) => this.keys(s).filter((id) => str(this.get([s, id, "in"])) === room));
  }

  // ---------------------------------------------------------- writing

  /** Sets a value, or removes it when `value` is undefined (and tidies empty maps away). */
  set(path: (string | number)[], value: unknown) {
    if (value === undefined || value === "" || value === false) {
      if (this.doc.hasIn(path)) this.doc.deleteIn(path);
      // Don't leave `state: {}` or `exits: {}` behind.
      for (let i = path.length - 1; i > 1; i--) {
        const parent = this.doc.getIn(path.slice(0, i), true);
        if (isMap(parent) && parent.items.length === 0) this.doc.deleteIn(path.slice(0, i));
        else break;
      }
      return;
    }
    if (this.doc.hasIn(path)) {
      const node = this.doc.getIn(path, true);
      // Keep the scalar node, so its comment and quoting style survive.
      if (isScalar(node) && typeof value !== "object") {
        node.value = value;
        return;
      }
    }
    this.doc.setIn(path, this.doc.createNode(value));
  }

  /** A new ID not used by anything else. */
  freshId(base: string): string {
    const used = this.allIds();
    if (!used.has(base)) return base;
    for (let i = 2; ; i++) if (!used.has(`${base}_${i}`)) return `${base}_${i}`;
  }

  addRoom(name = "New room"): string {
    const id = this.freshId(slug(name));
    if (!this.map(["rooms"])) this.doc.setIn(["rooms"], this.doc.createNode({}));
    this.doc.setIn(["rooms", id], this.doc.createNode({ name }));
    if (!this.start()) this.set(["game", "start"], id);
    return id;
  }

  /** Renames a room's ID, and every reference to it the editor can see. */
  renameRoom(old: string, next: string): string | null {
    next = slug(next);
    if (next === old) return old;
    if (this.allIds().has(next)) return null;
    const rooms = this.map(["rooms"]);
    const pair = rooms?.items.find((p) => (isScalar(p.key) ? p.key.value : p.key) === old) as Pair | undefined;
    if (!pair) return null;
    pair.key = new Scalar(next);
    this.replaceRefs(old, next);
    return next;
  }

  /** Points every exit, `in:`, `start` and door side list at `next` instead of `old`. */
  private replaceRefs(old: string, next: string | null) {
    const swap = (node: unknown, parent: YAMLMap | YAMLSeq, key: unknown) => {
      if (isScalar(node) && node.value === old) {
        if (next === null) parent.delete(key);
        else node.value = next;
      }
    };
    for (const id of this.roomIds()) {
      const exits = this.map(["rooms", id, "exits"]);
      if (!exits) continue;
      for (const p of [...exits.items]) {
        const v = p.value;
        if (isScalar(v)) swap(v, exits, p.key);
        else if (isMap(v)) swap(v.get("to", true), v, "to");
        else if (isSeq(v)) for (const [i, item] of [...v.items].entries()) isMap(item) ? swap(item.get("to", true), item, "to") : swap(item, v, i);
      }
    }
    for (const s of ["objects", "characters"]) {
      for (const id of this.keys(s)) {
        const m = this.map([s, id]);
        if (m) swap(m.get("in", true), m, "in");
        // Door sides: blocked/hidden can be a room or a list of rooms.
        const state = this.map([s, id, "state"]);
        for (const k of ["blocked", "hidden"]) {
          const v = state?.get(k, true);
          if (isScalar(v)) swap(v, state!, k);
          if (isSeq(v)) for (const [i, item] of [...v.items].entries()) swap(item, v, i);
        }
      }
    }
    const start = this.doc.getIn(["game", "start"], true);
    if (isScalar(start) && start.value === old) {
      if (next === null) {
        const other = this.roomIds()[0];
        if (other) start.value = other;
        else this.doc.deleteIn(["game", "start"]);
      } else start.value = next;
    }
  }

  /** Deletes a room and the exits into it; things in it are moved off-stage. */
  deleteRoom(id: string) {
    for (const r of this.roomIds()) {
      for (const e of this.exits(r)) if (e.to === id && e.kind !== "advanced") this.removeExit(r, e.dir);
    }
    for (const s of ["objects", "characters"]) {
      for (const t of this.keys(s)) if (str(this.get([s, t, "in"])) === id) this.doc.deleteIn([s, t, "in"]);
    }
    this.doc.deleteIn(["rooms", id]);
    this.replaceRefs(id, null);
  }

  /** A plain exit, or one through a door. Replaces whatever was that way. */
  setExit(room: string, dir: Direction, to: string, door?: string) {
    this.set(["rooms", room, "exits", dir], door ? { to, through: door } : to);
  }

  setBlocked(room: string, dir: Direction, message: string) {
    this.set(["rooms", room, "exits", dir], message || "You can't go that way.");
  }

  removeExit(room: string, dir: Direction) {
    this.set(["rooms", room, "exits", dir], undefined);
  }

  /** The exit from `to` that leads back to `from`, if there is one. */
  wayBack(from: string, to: string, dir: Direction): Exit | undefined {
    const exits = this.exits(to).filter((e) => e.to === from);
    return exits.find((e) => e.dir === OPPOSITE[dir]) ?? exits[0];
  }

  /**
   * Connects two rooms: `from` going `dir` reaches `to`, and, if `twoWay`,
   * `to` going the opposite way comes back (when that way is free).
   */
  connect(from: string, dir: Direction, to: string, twoWay = true) {
    this.setExit(from, dir, to);
    if (twoWay && !this.wayBack(from, to, dir) && !this.exit(to, OPPOSITE[dir])) this.setExit(to, OPPOSITE[dir], from);
  }

  /** Puts a new door on the exit `room` → `dir`, on both sides. Returns its ID. */
  addDoor(room: string, dir: Direction, name = "door"): string | null {
    const e = this.exit(room, dir);
    if (!e?.to || e.kind === "advanced") return null;
    const id = this.freshId(slug(`${this.room(room)?.name ?? room} ${name}`, "door"));
    if (!this.map(["objects"])) this.doc.setIn(["objects"], this.doc.createNode({}));
    const node = this.doc.createNode({ name, in: room, traits: ["door", "openable", "scenery"] }) as YAMLMap;
    (node.get("traits", true) as unknown as YAMLSeq).flow = true;
    this.doc.setIn(["objects", id], node);
    this.setExit(room, dir, e.to, id);
    // The way back goes through the same door (the compiler would add it, but say so).
    const back = this.wayBack(room, e.to, dir);
    if (back && back.kind === "plain") this.setExit(e.to, back.dir, room, id);
    else if (!back && !this.exit(e.to, OPPOSITE[dir])) this.setExit(e.to, OPPOSITE[dir], room, id);
    return id;
  }

  /** Takes a door away: its exits become plain ones, and the door object is deleted. */
  removeDoor(id: string) {
    for (const r of this.roomIds()) for (const e of this.exits(r)) if (e.door === id && e.to && e.kind === "door") this.setExit(r, e.dir, e.to);
    this.doc.deleteIn(["objects", id]);
  }

  setTraits(path: string[], add: string[], remove: string[]) {
    const now = this.traits(path).filter((t) => !remove.includes(t));
    for (const t of add) if (!now.includes(t)) now.push(t);
    const node = this.doc.getIn([...path, "traits"], true);
    if (isSeq(node)) {
      node.items = now.map((t) => new Scalar(t));
      return;
    }
    const seq = this.doc.createNode(now) as YAMLSeq;
    seq.flow = true;
    this.doc.setIn([...path, "traits"], seq);
  }

  /** Sets a door's starting state: open, closed, or locked. */
  setDoorState(id: string, s: "open" | "closed" | "locked") {
    const p = ["objects", id];
    if (s === "locked") this.setTraits(p, ["openable", "lockable"], []);
    this.set([...p, "state", "open"], s === "open" ? true : undefined);
    this.set([...p, "state", "locked"], s === "locked" ? true : undefined);
    if (s !== "locked" && !this.get([...p, "unlocks_with"])) this.setTraits(p, [], ["lockable"]);
  }

  /** A state map in flow style (`state: { locked: true }`), like the examples. */
  tidyState(path: string[]) {
    const s = this.doc.getIn([...path, "state"], true);
    if (isMap(s)) s.flow = true;
  }
}
