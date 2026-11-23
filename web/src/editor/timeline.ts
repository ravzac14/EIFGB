// The timer timeline: when each timer can go off, and what it does, in
// turns and (with a clock) hours. Worked out from the file alone, as if
// nothing else happened: a timer started by an action counts from then.

import type { Model } from "./model.ts";

export interface TimerPlan {
  id: string;
  /** "once, after 5 actions", "every 10 to 15 actions", "each day at 7:00 pm". */
  schedule: string;
  /** "with the game", or what starts it. */
  starts: string;
  /** Counting from the start of the game (true), or from when it's started. */
  fromStart: boolean;
  /** Where it can go off, in actions from when it starts: a moment, or a span for a range. */
  marks: { from: number; to: number }[];
  /** What it does, in order: "says “…”", "runs a script", "ends the game (drowned)". */
  does: string[];
  /** Why it might not: "only if …", "1 in 3". */
  unless: string[];
  /** What stops it. */
  stops?: string;
}

export interface Timeline {
  timers: TimerPlan[];
  /** How many actions the timeline shows. */
  span: number;
  /** With a clock: minutes after midnight at the start, and minutes per action. */
  clock?: { start: number; perTurn: number };
}

/** "5", 5, "5-10", "5 to 10", "5..10" → [5, 10]. */
function range(v: unknown): [number, number] | null {
  if (typeof v === "number" && v >= 1) return [v, v];
  if (typeof v !== "string") return null;
  const m = /^\s*(\d+)\s*(?:-|to|\.\.)\s*(\d+)\s*$/.exec(v) ?? /^\s*(\d+)\s*$/.exec(v);
  if (!m) return null;
  const a = Number(m[1]);
  const b = Number(m[2] ?? m[1]);
  return a >= 1 && b >= a ? [a, b] : null;
}

/** Minutes after midnight: "7:30", "7:30 pm", "7pm", "noon", "midnight", 7. */
export function parseTime(v: unknown, dawn = 360, dusk = 1080): number | null {
  if (typeof v === "number" && v >= 0 && v < 24) return v * 60;
  if (typeof v !== "string") return null;
  const s = v.trim().toLowerCase().replace(/\./g, "");
  if (s === "noon") return 720;
  if (s === "midnight") return 0;
  if (s === "dawn") return dawn;
  if (s === "dusk") return dusk;
  const m = /^(\d{1,2})(?::(\d{2}))?\s*(am|pm)?$/.exec(s);
  if (!m) return null;
  let h = Number(m[1]);
  const min = Number(m[2] ?? 0);
  if (m[3] === "pm" && h < 12) h += 12;
  if (m[3] === "am" && h === 12) h = 0;
  return h < 24 && min < 60 ? h * 60 + min : null;
}

/** "7:05 am", "12:30 pm". */
export function timeText(minutes: number): string {
  const m = ((Math.round(minutes) % 1440) + 1440) % 1440;
  const h = Math.floor(m / 60);
  return `${h % 12 || 12}:${String(m % 60).padStart(2, "0")} ${h < 12 ? "am" : "pm"}`;
}

const actions = (n: number) => `${n} action${n === 1 ? "" : "s"}`;
const span = ([a, b]: [number, number]) => (a === b ? actions(a) : `${a} to ${b} actions`);
const quote = (s: string) => `“${s.length > 60 ? `${s.slice(0, 58).trimEnd()}…` : s.trim()}”`;

/** Rules like `started_by`, said shortly: "take lamp", "entering the cellar". */
function rule(v: unknown): string {
  if (typeof v === "string") return v.replace(/_/g, " ");
  if (Array.isArray(v)) return v.map(rule).join(", or ");
  if (v && typeof v === "object") {
    return Object.entries(v as Record<string, unknown>)
      .map(([k, x]) => `${k.replace(/_/g, " ")} ${rule(x)}`)
      .join(", ");
  }
  return String(v);
}

export function timeline(model: Model): Timeline {
  const clockOn = model.get(["clock"]) !== undefined;
  const dawn = parseTime(model.get(["clock", "dawn"])) ?? 360;
  const dusk = parseTime(model.get(["clock", "dusk"])) ?? 1080;
  const start = parseTime(model.get(["clock", "start"]), dawn, dusk) ?? 480;
  const perTurn = Math.max(1, Number(model.get(["clock", "minutes_per_turn"]) ?? 1) || 1);
  const timers: TimerPlan[] = [];
  let reach = 0;
  for (const id of model.ids("timers")) {
    // Plain values: a map or list comes back from the model as a YAML node.
    const g = (k: string) => {
      const v = model.get(["timers", id, k]) as { toJSON?: () => unknown } | undefined;
      return v && typeof v === "object" && typeof v.toJSON === "function" ? v.toJSON() : v;
    };
    const after = range(g("after"));
    const every = range(g("every"));
    const at = g("at");
    const marks: { from: number; to: number }[] = [];
    let schedule = "";
    const startedBy = g("started_by");
    const fromStart = g("running") !== false && startedBy === undefined;
    if (after) {
      schedule = `once, after ${span(after)}`;
      marks.push({ from: after[0], to: after[1] });
      reach = Math.max(reach, after[1]);
    } else if (every) {
      schedule = `every ${span(every)}`;
      reach = Math.max(reach, every[1] * 3);
    } else if (at !== undefined) {
      const t = parseTime(at, dawn, dusk);
      schedule = t === null ? `each day at ${String(at)}` : `each day at ${typeof at === "string" && /^(dawn|dusk)$/i.test(at) ? `${at} (${timeText(t)})` : timeText(t)}`;
      if (t !== null && clockOn) {
        // The first action after which the clock has passed it: tomorrow, if it's that time now.
        const wait = (((t - start) % 1440) + 1440) % 1440 || 1440;
        const first = Math.ceil(wait / perTurn);
        reach = Math.max(reach, first);
        marks.push({ from: first, to: first });
      }
    } else schedule = "never: it needs after, every or at";
    const does: string[] = [];
    if (typeof g("say") === "string") does.push(`says ${quote(g("say") as string)}`);
    if (typeof g("say_again") === "string") does.push(`then ${quote(g("say_again") as string)}`);
    if (g("heard_in") !== undefined) does.push(`heard only in ${rule(g("heard_in"))}`);
    if (g("do") !== undefined) does.push("runs a script");
    if (g("noise") !== undefined) does.push("makes a noise");
    if (typeof g("end") === "string") does.push(`ends the game (${g("end")})`);
    if (!does.length) does.push("nothing yet");
    const unless: string[] = [];
    if (g("when") !== undefined) unless.push(`only if ${String(g("when"))}`);
    if (g("chance") !== undefined) unless.push(`a ${String(g("chance"))} chance each time`);
    timers.push({
      id,
      schedule,
      starts: fromStart ? "with the game" : startedBy !== undefined ? `when the player does: ${rule(startedBy)}` : "when a script starts it",
      fromStart,
      marks,
      does,
      unless,
      stops: g("stopped_by") !== undefined ? rule(g("stopped_by")) : undefined,
    });
  }
  const shown = Math.max(20, Math.ceil((reach * 1.15) / 10) * 10);
  // Repeating timers: every time they'd go off, across what's shown.
  for (const t of timers) {
    const every = range(model.get(["timers", t.id, "every"]));
    if (every) for (let n = 1; every[0] * n <= shown && n <= 50; n++) t.marks.push({ from: every[0] * n, to: every[1] * n });
    const at = model.get(["timers", t.id, "at"]);
    const tm = parseTime(at, dawn, dusk);
    if (tm !== null && clockOn && t.marks.length) {
      const day = 1440 / perTurn;
      for (let n = 1; t.marks[0].from + n * day <= shown && n <= 50; n++) {
        const next = Math.ceil(t.marks[0].from + n * day);
        t.marks.push({ from: next, to: next });
      }
    }
  }
  return { timers, span: shown, ...(clockOn ? { clock: { start, perTurn } } : {}) };
}
