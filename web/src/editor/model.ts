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
  /** Things the way goes by (`via`): CLIMB THE STAIRS. */
  via?: string[];
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
const esc = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/**
 * How Lua code refers to something by its ID, rewritten for a rename:
 * `world.hall`, `vars.gold`, `end_game("won")` and so on. For game.yaml's
 * scripts and the project's .lua files alike.
 */
export function renameInCode(kind: RenameKind, old: string, next: string): (code: string) => string {
  const o = esc(old);
  const call = (fns: string[]) => new RegExp(`\\b(${fns.join("|")})(\\s*\\(\\s*)(["'])${o}\\3`, "g");
  const rules: [RegExp, string][] =
    kind === "entity"
      ? [
          [new RegExp(`\\bworld(\\s*)\\.(\\s*)${o}\\b`, "g"), `world$1.$2${next}`],
          [new RegExp(`\\bworld\\[(\\s*)(["'])${o}\\2(\\s*)\\]`, "g"), `world[$1$2${next}$2$3]`],
        ]
      : kind === "vars"
        ? [[new RegExp(`\\bvars(\\s*)\\.(\\s*)${o}\\b`, "g"), `vars$1.$2${next}`], [new RegExp(`\\bvars\\[(["'])${o}\\1\\]`, "g"), `vars[$1${next}$1]`]]
        : kind === "endings"
          ? [[call(["end_game"]), `$1$2$3${next}$3`]]
          : kind === "choices"
            ? [[call(["open_choice"]), `$1$2$3${next}$3`]]
            : kind === "timers"
              ? [[call(["start_timer", "stop_timer", "timer_left"]), `$1$2$3${next}$3`]]
              : kind === "goals"
                ? [[call(["start_goal", "complete_goal"]), `$1$2$3${next}$3`], [new RegExp(`\\bgoals(\\s*)\\.(\\s*)${o}\\b`, "g"), `goals$1.$2${next}`]]
                : [];
  return (code) => rules.reduce((s, [re, to]) => s.replace(re, to), code);
}

export type RenameKind = "entity" | "vars" | "endings" | "choices" | "timers" | "goals" | "verbs" | "hints";

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

/** An object, as the editor shows it. Doors are objects too, but live on the map's lines. */
export interface Thing {
  id: string;
  name: string;
  /** Where it starts: a room, a container or supporter, `player`, or null (nowhere yet). */
  in: string | null;
  traits: string[];
  description: string;
  synonyms: string[];
  adjectives: string[];
  hidden: boolean;
  /** `"search mat"` rules, as written. */
  revealedBy: string[];
  open: boolean;
  locked: boolean;
  lit: boolean;
  key: string | null;
  size: number | null;
  capacity: number | null;
}

/** Kinds of thing a newcomer can start from. */
export const KINDS = {
  carry: { name: "new thing", traits: ["portable"] },
  scenery: { name: "new feature", traits: ["scenery"] },
  container: { name: "box", traits: ["container", "openable"] },
  supporter: { name: "table", traits: ["supporter", "scenery"] },
  key: { name: "key", traits: ["portable"] },
  light: { name: "lamp", traits: ["portable", "light_source"] },
  picture: { name: "photograph", traits: ["portable"] },
} as const;
export type Kind = keyof typeof KINDS;

export const BEHAVIORS = ["stationary", "conversationalist", "wanderer", "merchant", "hunter", "guard", "follower", "defender"] as const;
export type Behavior = (typeof BEHAVIORS)[number];

/** A character, as the editor shows it. */
export interface Character {
  id: string;
  name: string;
  in: string | null;
  description: string;
  synonyms: string[];
  /** As written; null means the default (stationary, or conversationalist with `talk`). */
  behavior: string | null;
  faction: string | null;
  talk: string | null;
  health: number | null;
  damage: number | null;
}

/** People a newcomer can start from. */
export const PEOPLE = {
  talker: { name: "stranger", fields: { behavior: "conversationalist" } },
  merchant: { name: "shopkeeper", fields: { behavior: "merchant" } },
  wanderer: { name: "traveller", fields: { behavior: "wanderer" } },
  follower: { name: "companion", fields: { behavior: "follower" } },
  hunter: { name: "wolf", fields: { behavior: "hunter", health: 5, damage: 1 } },
  guard: { name: "guard", fields: { behavior: "guard", health: 8, damage: 2 } },
  still: { name: "person", fields: {} },
} as const;
export type Person = keyof typeof PEOPLE;

export interface Faction {
  id: string;
  /** Other factions, and/or `player`. */
  hostileTo: string[];
  members: string[];
}

export interface Handler {
  /** As written: `before_take`, `enter`, `after_eat`… */
  event: string;
  code: string;
}

export interface ChoiceOption {
  text: string;
  when: string;
  say: string;
  do: string;
  goto: string;
  end: boolean;
}
export interface ChoiceNode {
  id: string;
  text: string;
  do: string;
  goto: string;
  options: ChoiceOption[];
}
export interface Choice {
  id: string;
  start: string;
  nodes: ChoiceNode[];
}

export const EVENTS = {
  game: ["start", "turn", "parse_failed", "died"],
  room: ["enter", "leave", "turn"],
  character: ["defeated"],
  thing: [] as string[],
} as const;

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

  /** Dialogues and menus, from `choices`. */
  choiceIds(): string[] {
    return this.keys("choices");
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
        const rawVia = v.get("via");
        const via = isSeq(rawVia) ? rawVia.items.map((i) => str(i)).filter((s): s is string => !!s) : str(rawVia) ? [str(rawVia)!] : [];
        const simple = v.items.every((i) => ["to", "through", "via"].includes(String(isScalar(i.key) ? i.key.value : i.key)));
        if (simple && to) out.push({ dir, to, door: through, via, kind: through ? "door" : "plain" });
        else out.push({ dir, to, door: through, blocked, via, kind: "advanced" });
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

  // --------------------------------------------------------- objects

  objectIds(): string[] {
    return this.keys("objects");
  }

  thing(id: string): Thing | null {
    const m = this.map(["objects", id]);
    if (!m) return null;
    const p = ["objects", id];
    const list = (k: string) => {
      const v = this.doc.getIn([...p, k], true);
      if (isSeq(v)) return v.items.map((i) => String(isScalar(i) ? i.value : i));
      const s = str(v);
      return s ? [s] : [];
    };
    const state = (k: string) => this.get([...p, "state", k]) === true;
    const num = (k: string) => {
      const v = this.get([...p, k]);
      return typeof v === "number" ? v : null;
    };
    return {
      id,
      name: str(m.get("name")) ?? id,
      in: str(m.get("in")) ?? null,
      traits: this.traits(p),
      description: str(m.get("description")) ?? "",
      synonyms: list("synonyms"),
      adjectives: list("adjectives"),
      hidden: state("hidden"),
      revealedBy: list("revealed_by"),
      open: state("open"),
      locked: state("locked"),
      lit: state("lit"),
      key: str(m.get("unlocks_with")) ?? null,
      size: num("size"),
      capacity: num("capacity"),
    };
  }

  /** Objects that aren't doors. */
  things(): Thing[] {
    return this.objectIds()
      .map((id) => this.thing(id))
      .filter((t): t is Thing => t !== null && !t.traits.includes("door"));
  }

  /** What starts directly in or on `place` (a room, a thing, or `player`). */
  contents(place: string | null): Thing[] {
    return this.things().filter((t) => t.in === place);
  }

  /** Whether things can go in or on `id`: rooms, the player, containers and supporters. */
  canHold(id: string): boolean {
    if (id === "player" || this.room(id) || this.character(id)) return true;
    const t = this.traits(["objects", id]);
    return t.includes("container") || t.includes("supporter");
  }

  /** The room a thing is in, however deep (or "player", or null). */
  roomOf(id: string): string | null {
    let at: string | null = id;
    for (let i = 0; i < 100 && at; i++) {
      if (at === "player" || this.room(at)) return at;
      at = this.thing(at)?.in ?? this.character(at)?.in ?? null;
    }
    return null;
  }

  /**
   * Moves a thing to a room, into or onto another thing, to `player`, or to
   * nowhere (null). False if that can't hold things, or is inside it.
   */
  moveThing(id: string, to: string | null): boolean {
    if (to !== null) {
      if (to === id || !this.canHold(to)) return false;
      for (let at: string | null = to, i = 0; at && i < 100; i++) {
        if (at === id) return false; // into something it contains
        at = this.thing(at)?.in ?? null;
      }
    }
    this.set(["objects", id, "in"], to ?? undefined);
    return true;
  }

  addThing(kind: Kind, place: string | null, name?: string): string {
    const k = KINDS[kind];
    const n = name ?? k.name;
    const id = this.freshId(slug(n, "thing"));
    if (!this.map(["objects"])) this.doc.setIn(["objects"], this.doc.createNode({}));
    const fields: Record<string, unknown> = { name: n };
    if (place) fields.in = place;
    const node = this.doc.createNode({ ...fields, traits: [...k.traits] }) as YAMLMap;
    (node.get("traits", true) as unknown as YAMLSeq).flow = true;
    this.doc.setIn(["objects", id], node);
    return id;
  }

  /** Sets a list field (synonyms, adjectives) from text like "rug, mat". */
  setList(path: (string | number)[], items: string[]) {
    const clean = items.map((s) => s.trim()).filter(Boolean);
    if (!clean.length) return this.set(path, undefined);
    const seq = this.doc.createNode(clean) as YAMLSeq;
    seq.flow = true;
    this.doc.setIn(path, seq);
  }

  setTrait(id: string, trait: string, on: boolean) {
    this.setTraits(["objects", id], on ? [trait] : [], on ? [] : [trait]);
    // Things that depend on it go with it.
    if (!on && trait === "openable") {
      this.setTraits(["objects", id], [], ["lockable"]);
      this.set(["objects", id, "state", "open"], undefined);
      this.set(["objects", id, "state", "locked"], undefined);
    }
    if (!on && trait === "lockable") this.set(["objects", id, "state", "locked"], undefined);
    if (!on && trait === "light_source") this.set(["objects", id, "state", "lit"], undefined);
    if (on && trait === "lockable") this.setTraits(["objects", id], ["openable"], []);
    if (this.traits(["objects", id]).length === 0) this.set(["objects", id, "traits"], undefined);
  }

  /** "Can the player pick it up?": yes (portable), no and it's part of the room (scenery), or no. */
  setPortability(id: string, how: "portable" | "scenery" | "fixed") {
    this.setTraits(["objects", id], how === "fixed" ? [] : [how], ["portable", "scenery"].filter((t) => t !== how));
    if (this.traits(["objects", id]).length === 0) this.set(["objects", id, "traits"], undefined);
  }

  setState(id: string, key: string, on: boolean) {
    this.set(["objects", id, "state", key], on || undefined);
    this.tidyState(["objects", id]);
  }

  /**
   * The last rename, for what's outside the game file: the editor rewrites
   * the .lua files with `code`, and moves a room's place on the map.
   */
  lastRename: { kind: RenameKind; old: string; next: string; code: (s: string) => string } | null = null;

  /** Renames a thing's ID, and everything that refers to it. */
  renameThing(old: string, next: string): string | null {
    next = slug(next, "thing");
    if (next === old) return old;
    if (this.allIds().has(next)) return null;
    const objects = this.map(["objects"]);
    const pair = objects?.items.find((p) => (isScalar(p.key) ? p.key.value : p.key) === old) as Pair | undefined;
    if (!pair) return null;
    pair.key = new Scalar(next);
    this.renameEntityRefs(old, next);
    return next;
  }

  /**
   * Every reference to an entity's ID: exits (to, through, via), `in`,
   * keys, characters' things and areas, noises, door sides, the start
   * room, rules ("search mat"), and Lua code anywhere in the file.
   */
  private renameEntityRefs(old: string, next: string) {
    const REFS = new Set(["to", "through", "via", "in", "unlocks_with", "area", "at", "tame_with", "likes", "fears", "blocked", "hidden", "heard_in"]);
    const RULES = new Set(["revealed_by", "started_by", "stopped_by"]);
    const code = renameInCode("entity", old, next);
    const rule = (s: string) => s.replace(new RegExp(`^(\\s*(?:\\([^)]*\\)|[\\w-]+)\\s+)${esc(old)}(\\s*)$`), `$1${next}$2`);
    const visit = (node: unknown, key: string | null) => {
      if (isMap(node)) {
        for (const p of node.items) {
          const k = String(isScalar(p.key) ? p.key.value : p.key);
          if (isScalar(p.value) || isSeq(p.value) || isMap(p.value)) visit(p.value, k);
        }
      } else if (isSeq(node)) {
        for (const item of node.items) visit(item, key);
      } else if (isScalar(node) && typeof node.value === "string") {
        const v = node.value;
        if (key && REFS.has(key)) {
          if (v === old) node.value = next;
        } else if (key && RULES.has(key)) node.value = rule(v);
        else {
          const out = code(v);
          if (out !== v) node.value = out;
        }
      }
    };
    visit(this.doc.contents, null);
    const start = this.doc.getIn(["game", "start"], true);
    if (isScalar(start) && start.value === old) start.value = next;
    // Exits written as just a room: `north: hall`.
    for (const id of this.roomIds()) {
      for (const p of this.map(["rooms", id, "exits"])?.items ?? []) {
        if (isScalar(p.value) && p.value.value === old) p.value.value = next;
        if (isSeq(p.value)) for (const i of p.value.items) if (isScalar(i) && i.value === old) i.value = next;
      }
    }
    this.lastRename = { kind: "entity", old, next, code };
  }

  /** Applies a code rewrite to every string in the game file. */
  private rewriteCode(code: (s: string) => string) {
    const visit = (node: unknown) => {
      if (isMap(node)) for (const p of node.items) visit(p.value);
      else if (isSeq(node)) for (const i of node.items) visit(i);
      else if (isScalar(node) && typeof node.value === "string") {
        const out = code(node.value);
        if (out !== node.value) node.value = out;
      }
    };
    visit(this.doc.contents);
  }

  /** Points `in:`, `unlocks_with` and `revealed_by` rules at `next` (or drops them, for null). */
  private retarget(old: string, next: string | null) {
    for (const s of ["objects", "characters"]) {
      for (const id of this.keys(s)) {
        const m = this.map([s, id]);
        if (!m) continue;
        for (const k of ["in", "unlocks_with"]) {
          const v = m.get(k, true);
          if (isScalar(v) && v.value === old) {
            if (next === null) m.delete(k);
            else v.value = next;
          }
        }
        // revealed_by: "search mat", or a list of them.
        const fix = (rule: unknown) => {
          if (!isScalar(rule) || typeof rule.value !== "string") return;
          const words = rule.value.trim().split(/\s+/);
          if (words.length === 2 && words[1] === old && next !== null) rule.value = `${words[0]} ${next}`;
        };
        const r = m.get("revealed_by", true);
        if (isSeq(r)) r.items.forEach(fix);
        else fix(r);
      }
    }
  }

  /** Deletes a thing; whatever was in or on it goes where it was. */
  deleteThing(id: string) {
    const where = this.thing(id)?.in ?? null;
    for (const t of this.contents(id)) this.set(["objects", t.id, "in"], where ?? undefined);
    this.retarget(id, null);
    this.doc.deleteIn(["objects", id]);
  }

  // ------------------------------------------------------ characters

  characterIds(): string[] {
    return this.keys("characters");
  }

  character(id: string): Character | null {
    const m = this.map(["characters", id]);
    if (!m) return null;
    const num = (k: string) => {
      const v = this.get(["characters", id, k]);
      return typeof v === "number" ? v : null;
    };
    const syn = this.doc.getIn(["characters", id, "synonyms"], true);
    return {
      id,
      name: str(m.get("name")) ?? id,
      in: str(m.get("in")) ?? null,
      description: str(m.get("description")) ?? "",
      synonyms: isSeq(syn) ? syn.items.map((i) => String(isScalar(i) ? i.value : i)) : [],
      behavior: str(m.get("behavior")) ?? null,
      faction: str(m.get("faction")) ?? null,
      talk: str(m.get("talk")) ?? null,
      health: num("health"),
      damage: num("damage"),
    };
  }

  characters(): Character[] {
    return this.characterIds()
      .map((id) => this.character(id))
      .filter((c): c is Character => c !== null);
  }

  /** The behaviour that applies, defaults included. */
  behaviorOf(c: Character): Behavior {
    const b = c.behavior ?? (c.talk ? "conversationalist" : "stationary");
    return (BEHAVIORS as readonly string[]).includes(b) ? (b as Behavior) : "stationary";
  }

  /** Whether a character will attack the player: by behaviour, or by its faction. */
  hostile(c: Character): boolean {
    const b = this.behaviorOf(c);
    if (b === "hunter" || b === "guard") return true;
    return !!c.faction && this.faction(c.faction)?.hostileTo.includes("player") === true;
  }

  /** Characters who start in `room`. */
  peopleIn(room: string): Character[] {
    return this.characters().filter((c) => c.in === room);
  }

  addCharacter(kind: Person, room: string | null, name?: string): string {
    const k = PEOPLE[kind];
    const n = name ?? k.name;
    const id = this.freshId(slug(n, "character"));
    if (!this.map(["characters"])) this.doc.setIn(["characters"], this.doc.createNode({}));
    this.doc.setIn(["characters", id], this.doc.createNode({ name: n, ...(room ? { in: room } : {}), ...k.fields }));
    // Enemies need the player to have health to fight with.
    if ((kind === "hunter" || kind === "guard") && this.get(["player", "health"]) === undefined) {
      if (!this.map(["player"])) this.addSection("player", { health: 10 }, ["game", "settings", "vars"]);
      else this.set(["player", "health"], 10);
    }
    return id;
  }

  /** Moves a character to a room (or nowhere). */
  moveCharacter(id: string, room: string | null) {
    this.set(["characters", id, "in"], room ?? undefined);
  }

  renameCharacter(old: string, next: string): string | null {
    next = slug(next, "character");
    if (next === old) return old;
    if (this.allIds().has(next)) return null;
    const pair = this.map(["characters"])?.items.find((p) => (isScalar(p.key) ? p.key.value : p.key) === old) as Pair | undefined;
    if (!pair) return null;
    pair.key = new Scalar(next);
    this.renameEntityRefs(old, next);
    return next;
  }

  /** Deletes a character; what it carries is left in its room. */
  deleteCharacter(id: string) {
    const room = this.character(id)?.in ?? null;
    for (const o of this.objectIds()) if (str(this.get(["objects", o, "in"])) === id) this.set(["objects", o, "in"], room ?? undefined);
    this.doc.deleteIn(["characters", id]);
  }

  // -------------------------------------------------------- factions

  factionIds(): string[] {
    return this.keys("factions");
  }

  faction(id: string): Faction | null {
    if (!this.doc.hasIn(["factions", id])) return null;
    const h = this.doc.getIn(["factions", id, "hostile_to"], true);
    return {
      id,
      hostileTo: isSeq(h) ? h.items.map((i) => String(isScalar(i) ? i.value : i)) : [],
      members: this.characters().filter((c) => c.faction === id).map((c) => c.id),
    };
  }

  factions(): Faction[] {
    return this.factionIds()
      .map((id) => this.faction(id))
      .filter((f): f is Faction => f !== null);
  }

  addFaction(name: string): string {
    let id = slug(name, "faction");
    const used = new Set(this.factionIds());
    if (used.has(id)) for (let i = 2; ; i++) if (!used.has(`${id}_${i}`)) { id = `${id}_${i}`; break; }
    if (!this.map(["factions"])) this.doc.setIn(["factions"], this.doc.createNode({}));
    this.doc.setIn(["factions", id], this.doc.createNode({}));
    return id;
  }

  /** `a` attacks `b` (another faction, or "player") on sight, or stops. */
  setHostile(a: string, b: string, on: boolean) {
    const now = this.faction(a)?.hostileTo ?? [];
    const next = on ? [...new Set([...now, b])] : now.filter((x) => x !== b);
    if (!this.map(["factions", a])) this.doc.setIn(["factions", a], this.doc.createNode({}));
    this.setList(["factions", a, "hostile_to"], next);
  }

  /** Every place a faction is named: members, hostility, disguises, eaten_by. */
  private factionRefs(fn: (node: unknown, parent: YAMLMap | YAMLSeq, key: unknown) => void) {
    for (const s of ["characters", "objects"]) {
      for (const id of this.keys(s)) {
        const m = this.map([s, id]);
        if (!m) continue;
        for (const k of ["faction", "disguise"]) if (m.has(k)) fn(m.get(k, true), m, k);
        const e = m.get("eaten_by", true);
        if (isSeq(e)) [...e.items].forEach((x, i) => fn(x, e, i));
      }
    }
    for (const f of this.factionIds()) {
      const h = this.doc.getIn(["factions", f, "hostile_to"], true);
      if (isSeq(h)) [...h.items].forEach((x, i) => fn(x, h, i));
    }
  }

  renameFaction(old: string, next: string): string | null {
    next = slug(next, "faction");
    if (next === old) return old;
    if (this.factionIds().includes(next) || next === "player") return null;
    const pair = this.map(["factions"])?.items.find((p) => (isScalar(p.key) ? p.key.value : p.key) === old) as Pair | undefined;
    if (!pair) return null;
    pair.key = new Scalar(next);
    this.factionRefs((n) => {
      if (isScalar(n) && n.value === old) n.value = next;
    });
    return next;
  }

  deleteFaction(id: string) {
    this.doc.deleteIn(["factions", id]);
    // Remove references, from the end of lists so indexes stay right.
    const drop: [YAMLMap | YAMLSeq, unknown][] = [];
    this.factionRefs((n, parent, key) => {
      if (isScalar(n) && n.value === id) drop.push([parent, key]);
    });
    for (const [parent, key] of drop.reverse()) parent.delete(key);
    for (const f of this.factionIds()) {
      const h = this.doc.getIn(["factions", f, "hostile_to"], true);
      if (isSeq(h) && !h.items.length) this.doc.deleteIn(["factions", f, "hostile_to"]);
    }
    if (!this.factionIds().length) this.doc.deleteIn(["factions"]);
  }

  // ----------------------------------------------------------- scripts

  /** IDs in a top-level section: timers, endings, choices, verbs, vars… */
  ids(section: string): string[] {
    return this.keys(section);
  }

  /** Handlers under `on:` at `owner` (e.g. ["objects", "pie"], or [] for game-wide). */
  handlers(owner: string[]): Handler[] {
    const m = this.map([...owner, "on"]);
    if (!m) return [];
    return m.items.map((p) => ({ event: String(isScalar(p.key) ? p.key.value : p.key), code: str(p.value) ?? "" }));
  }

  /** Sets a handler's code; empty code removes it. A multi-line script is written as a block. */
  setHandler(owner: string[], event: string, code: string) {
    const path = [...owner, "on", event];
    if (!code.trim()) {
      this.set(path, undefined);
      if (!owner.length && !this.map(["on"])?.items.length) this.doc.deleteIn(["on"]);
      return;
    }
    if (!owner.length && !this.map(["on"])) this.addSection("on", {}, ["timers", "choices", "endings", "characters", "objects", "rooms"]);
    const node = this.doc.getIn(path, true);
    const text = code.includes("\n") && !code.endsWith("\n") ? `${code}\n` : code;
    if (isScalar(node)) {
      node.value = text;
      node.type = text.includes("\n") ? Scalar.BLOCK_LITERAL : undefined;
      return;
    }
    const s = new Scalar(text);
    if (text.includes("\n")) s.type = Scalar.BLOCK_LITERAL;
    this.doc.setIn(path, s);
  }

  /** Renames a handler (keeps its place and code). */
  renameHandler(owner: string[], from: string, to: string): boolean {
    const m = this.map([...owner, "on"]);
    if (!m || m.has(to)) return false;
    const pair = m.items.find((p) => (isScalar(p.key) ? p.key.value : p.key) === from) as Pair | undefined;
    if (!pair) return false;
    pair.key = new Scalar(to);
    return true;
  }

  /** A top-level section's new entry, with a fresh key, the section added if needed. */
  private addEntry(section: string, base: string, value: unknown, after: string[]): string {
    const used = new Set(this.keys(section));
    let id = slug(base, section.replace(/s$/, ""));
    if (used.has(id)) for (let i = 2; ; i++) if (!used.has(`${id}_${i}`)) { id = `${id}_${i}`; break; }
    if (!this.map([section])) this.addSection(section, {}, after);
    this.doc.setIn([section, id], this.doc.createNode(value));
    return id;
  }

  addEnding(name: string, kind: "win" | "loss" | "neutral" = "win"): string {
    return this.addEntry("endings", name, { kind, text: kind === "win" ? "You did it." : "The story ends here." }, ["timers", "choices", "characters", "objects", "rooms"]);
  }

  addTimer(name = "timer"): string {
    return this.addEntry("timers", name, { after: 5, say: "Something happens." }, ["characters", "objects", "rooms"]);
  }

  /** A puzzle's hints: a nudge, then the answer, offered always until given a `when`. */
  addHint(name = "hint"): string {
    const id = this.addEntry("hints", name, {}, ["timers", "endings", "choices", "characters", "objects", "rooms"]);
    this.setHintSteps(id, ["A nudge: what should the player notice?", "The answer: what to type."]);
    return id;
  }

  /** A hint's steps, one a line in the file, as written (empty ones too, while being typed). */
  setHintSteps(id: string, steps: string[]) {
    const seq = this.doc.createNode(steps) as YAMLSeq;
    seq.flow = false;
    this.doc.setIn(["hints", id, "steps"], seq);
  }

  /** A scene: a named stretch of the story, from a condition to another. */
  addScene(title = "A new scene"): string {
    return this.addEntry("scenes", title, { title, when: "false", done: "false" }, ["goals", "hints", "timers", "endings", "choices", "characters", "objects", "rooms"]);
  }

  /** A goal for the journal, named from its first few words. */
  addGoal(text = "Something to do."): string {
    const name = text.replace(/[^\p{L}\p{N}\s]/gu, " ").trim().split(/\s+/).slice(0, 3).join(" ") || "goal";
    return this.addEntry("goals", name, { text }, ["hints", "timers", "endings", "choices", "characters", "objects", "rooms"]);
  }

  /** Starts keeping the time of day, from 8:00 in the morning. */
  addClock() {
    if (!this.map(["clock"])) this.addSection("clock", { start: "8:00", minutes_per_turn: 1 }, ["settings", "presentation", "game"]);
  }

  /** Moves an entry of a top-level section earlier (-1) or later (+1): for hints, whose order matters. */
  moveEntry(section: string, id: string, by: -1 | 1): boolean {
    const m = this.map([section]);
    if (!m) return false;
    const i = m.items.findIndex((p) => (isScalar(p.key) ? p.key.value : p.key) === id);
    const j = i + by;
    if (i < 0 || j < 0 || j >= m.items.length) return false;
    [m.items[i], m.items[j]] = [m.items[j], m.items[i]];
    return true;
  }

  addVerb(word: string): string {
    const id = slug(word, "verb");
    const added = this.addEntry("verbs", id, { patterns: [`${id.replace(/_/g, " ")} {target}`], default: "Nothing happens." }, ["characters", "objects", "rooms"]);
    this.setPatterns(added, this.list(["verbs", added, "patterns"]));
    return added;
  }

  /** A verb's patterns, written like the examples: ["ring {target}", "ring {target} with {tool}"]. */
  setPatterns(verb: string, patterns: string[]) {
    const clean = patterns.map((s) => s.trim()).filter(Boolean);
    const seq = this.doc.createNode(clean.map((s) => Object.assign(new Scalar(s), { type: Scalar.QUOTE_DOUBLE }))) as YAMLSeq;
    seq.flow = true;
    this.doc.setIn(["verbs", verb, "patterns"], seq);
  }

  addVar(name: string, value: unknown): string {
    return this.addEntry("vars", name, value, ["settings", "game"]);
  }

  /** Deletes an ending, timer, choice, verb or variable, and the references to it the editor can see. */
  deleteEntry(section: string, id: string) {
    this.doc.deleteIn([section, id]);
    if (!this.keys(section).length) this.doc.deleteIn([section]);
    if (section === "endings") for (const t of this.keys("timers")) if (str(this.get(["timers", t, "end"])) === id) this.doc.deleteIn(["timers", t, "end"]);
    if (section === "choices") for (const c of this.characterIds()) if (str(this.get(["characters", c, "talk"])) === id) this.doc.deleteIn(["characters", c, "talk"]);
  }

  /** Renames an ending, timer, choice or verb, and the references the editor can see (not Lua). */
  renameEntry(section: string, old: string, next: string): string | null {
    next = slug(next, section.replace(/s$/, ""));
    if (next === old) return old;
    if (this.keys(section).includes(next)) return null;
    const pair = this.map([section])?.items.find((p) => (isScalar(p.key) ? p.key.value : p.key) === old) as Pair | undefined;
    if (!pair) return null;
    pair.key = new Scalar(next);
    const swap = (path: (string | number)[]) => {
      const n = this.doc.getIn(path, true);
      if (isScalar(n) && n.value === old) n.value = next;
    };
    if (section === "endings") for (const t of this.keys("timers")) swap(["timers", t, "end"]);
    if (section === "choices") for (const c of this.characterIds()) swap(["characters", c, "talk"]);
    // A verb's handlers, everywhere: `on: { ring: ... }`.
    if (section === "verbs") {
      const owners: string[][] = [[], ...this.roomIds().map((r) => ["rooms", r]), ...this.objectIds().map((o) => ["objects", o]), ...this.characterIds().map((c) => ["characters", c])];
      for (const o of owners) {
        for (const phase of ["", "before_", "after_"]) {
          const p = this.map([...o, "on"])?.items.find((i) => (isScalar(i.key) ? i.key.value : i.key) === `${phase}${old}`);
          if (p) p.key = new Scalar(`${phase}${next}`);
        }
      }
    }
    const kind = section as RenameKind;
    const code = renameInCode(kind, old, next);
    this.rewriteCode(code);
    this.lastRename = { kind, old, next, code };
    return next;
  }

  // ----------------------------------------------------------- choices

  choice(id: string): Choice | null {
    const m = this.map(["choices", id]);
    if (!m) return null;
    const nodes = this.map(["choices", id, "nodes"]);
    const s = (v: unknown) => str(v) ?? "";
    return {
      id,
      start: s(m.get("start")),
      nodes: (nodes?.items ?? []).map((p) => {
        const nid = String(isScalar(p.key) ? p.key.value : p.key);
        const n = isMap(p.value) ? p.value : null;
        const opts = n?.get("options", true);
        return {
          id: nid,
          text: s(n?.get("text")),
          do: s(n?.get("do")),
          goto: s(n?.get("goto")),
          options: isSeq(opts)
            ? opts.items.map((o) => {
                const om = isMap(o) ? o : null;
                return { text: s(om?.get("text")), when: s(om?.get("when")), say: s(om?.get("say")), do: s(om?.get("do")), goto: s(om?.get("goto")), end: om?.get("end") === true };
              })
            : [],
        };
      }),
    };
  }

  addChoice(name: string): string {
    return this.addEntry(
      "choices",
      name,
      { start: "start", nodes: { start: { text: "What do you say?", options: [{ text: "Goodbye.", end: true }] } } },
      ["characters", "objects", "rooms"],
    );
  }

  addNode(choice: string): string {
    const used = new Set(this.choice(choice)?.nodes.map((n) => n.id) ?? []);
    let id = "node";
    for (let i = 2; used.has(id); i++) id = `node_${i}`;
    this.doc.setIn(["choices", choice, "nodes", id], this.doc.createNode({ text: "", options: [{ text: "Back.", goto: this.choice(choice)?.start || "start" }] }));
    return id;
  }

  renameNode(choice: string, old: string, next: string): string | null {
    next = slug(next, "node");
    const c = this.choice(choice);
    if (!c || next === old) return old;
    if (c.nodes.some((n) => n.id === next)) return null;
    const pair = this.map(["choices", choice, "nodes"])?.items.find((p) => (isScalar(p.key) ? p.key.value : p.key) === old) as Pair | undefined;
    if (!pair) return null;
    pair.key = new Scalar(next);
    const swap = (path: (string | number)[]) => {
      const n = this.doc.getIn(path, true);
      if (isScalar(n) && n.value === old) n.value = next;
    };
    swap(["choices", choice, "start"]);
    for (const n of this.choice(choice)!.nodes) {
      swap(["choices", choice, "nodes", n.id, "goto"]);
      n.options.forEach((_, i) => swap(["choices", choice, "nodes", n.id, "options", i, "goto"]));
    }
    return next;
  }

  deleteNode(choice: string, id: string) {
    this.doc.deleteIn(["choices", choice, "nodes", id]);
    const c = this.choice(choice);
    if (!c) return;
    // Ways into it end the conversation instead.
    for (const n of c.nodes) {
      if (n.goto === id) this.doc.deleteIn(["choices", choice, "nodes", n.id, "goto"]);
      n.options.forEach((o, i) => {
        if (o.goto === id) {
          this.doc.deleteIn(["choices", choice, "nodes", n.id, "options", i, "goto"]);
          this.doc.setIn(["choices", choice, "nodes", n.id, "options", i, "end"], true);
        }
      });
    }
    if (c.start === id && c.nodes[0]) this.set(["choices", choice, "start"], c.nodes[0].id);
  }

  addOption(choice: string, node: string) {
    const path = ["choices", choice, "nodes", node, "options"];
    const seq = this.doc.getIn(path, true);
    const item = this.doc.createNode({ text: "Something to say.", end: true });
    if (isSeq(seq)) seq.items.push(item);
    else this.doc.setIn(path, this.doc.createNode([{ text: "Something to say.", end: true }]));
  }

  removeOption(choice: string, node: string, i: number) {
    const path = ["choices", choice, "nodes", node, "options"];
    this.doc.deleteIn([...path, i]);
    const seq = this.doc.getIn(path, true);
    if (isSeq(seq) && !seq.items.length) this.doc.deleteIn(path);
  }

  /** Where an option leads: another node, the end, or (empty) back to this node's text. */
  setOptionNext(choice: string, node: string, i: number, next: { goto?: string; end?: boolean }) {
    const p = ["choices", choice, "nodes", node, "options", i];
    this.set([...p, "goto"], next.goto || undefined);
    if (next.end) this.doc.setIn([...p, "end"], true);
    else this.set([...p, "end"], undefined);
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

  /** A list of words at `path` (`[a, b]`, or a single word), or []. */
  list(path: (string | number)[]): string[] {
    const v = this.doc.getIn(path, true);
    if (isSeq(v)) return v.items.map((i) => String(isScalar(i) ? i.value : i));
    const s = str(v);
    return s ? [s] : [];
  }

  /** Adds a top-level section after the last of `after` that exists (or at the end). */
  addSection(key: string, value: unknown, after: string[]) {
    const root = this.doc.contents;
    if (!isMap(root)) return this.doc.setIn([key], this.doc.createNode(value));
    const at = Math.max(-1, ...after.map((k) => root.items.findIndex((p) => (isScalar(p.key) ? p.key.value : p.key) === k)));
    const pair = this.doc.createPair(key, value);
    if (isMap(pair.value)) pair.value.flow = false;
    // A blank line before it, like the sections around it.
    (pair.key as Scalar).spaceBefore = true;
    root.items.splice(at + 1, 0, pair as Pair);
  }

  /** A true/false field with a default: the default is written by leaving it out. */
  setBool(path: (string | number)[], value: boolean, byDefault: boolean) {
    if (value === byDefault) this.set(path, undefined);
    else this.doc.setIn(path, value);
  }

  /** A true/false field, its default applied. */
  bool(path: (string | number)[], byDefault: boolean): boolean {
    const v = this.get(path);
    return typeof v === "boolean" ? v : byDefault;
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
    this.renameEntityRefs(old, next);
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
    const via = this.exit(room, dir)?.via ?? [];
    this.writeExit(room, dir, to, door, via);
  }

  /** What a way out goes by (`via`): things players can name to go that way. */
  setVia(room: string, dir: Direction, via: string[]) {
    const e = this.exit(room, dir);
    if (!e?.to || e.kind === "advanced") return;
    this.writeExit(room, dir, e.to, e.door, via);
  }

  private writeExit(room: string, dir: Direction, to: string, door: string | undefined, via: string[]) {
    if (!door && !via.length) return this.set(["rooms", room, "exits", dir], to);
    const v: Record<string, unknown> = { to };
    if (door) v.through = door;
    if (via.length === 1) v.via = via[0];
    else if (via.length) v.via = via;
    this.set(["rooms", room, "exits", dir], v);
  }

  /** A room's, object's or character's picture. */
  picture(owner: string[]): { file: string; alt: string } | null {
    const v = this.get([...owner, "picture"]);
    if (typeof v === "string") return { file: v, alt: "" };
    if (v && typeof v === "object" && typeof (v as { file?: unknown }).file === "string") {
      const p = v as { file: string; alt?: unknown };
      return { file: p.file, alt: typeof p.alt === "string" ? p.alt : "" };
    }
    return null;
  }

  /** Sets (or with null, removes) a picture, and what it shows. */
  setPicture(owner: string[], file: string | null, alt: string) {
    if (!file) return this.set([...owner, "picture"], undefined);
    this.set([...owner, "picture"], alt.trim() ? { file, alt } : file);
  }

  /** A small map's entries, as text: a routine's `"6:00": bakery`. */
  pairs(path: (string | number)[]): [string, string][] {
    const m = this.map(path);
    if (!m) return [];
    return m.items.map((p) => [String(isScalar(p.key) ? p.key.value : p.key), String(isScalar(p.value) ? p.value.value : (p.value ?? ""))]);
  }

  /** Sets a small map from entries (empty: removes it), in flow style: `{ "6:00": bakery }`. */
  setPairs(path: (string | number)[], pairs: [string, string][]) {
    const kept = pairs.filter(([k, v]) => k.trim() && v.trim());
    if (!kept.length) return this.set(path, undefined);
    const node = this.doc.createNode(Object.fromEntries(kept.map(([k, v]) => [k.trim(), v.trim()]))) as YAMLMap;
    node.flow = true;
    this.doc.setIn(path, node);
  }

  /** The game's aliases: its own words for words the game knows. */
  aliases(): [string, string][] {
    return this.keys("aliases").map((k) => [k, str(this.get(["aliases", k])) ?? ""]);
  }

  setAlias(word: string, means: string) {
    word = word.trim().toLowerCase();
    if (!word) return;
    if (!this.map(["aliases"])) this.addSection("aliases", {}, ["verbs", "messages", "settings", "game"]);
    this.doc.setIn(["aliases", word], means.trim().toLowerCase());
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
