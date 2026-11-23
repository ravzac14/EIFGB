// Hints drafted from the puzzle chain: for each step the player has to work
// out ("Search the worn mat: a brass key."), a hint that nudges first and
// gives the answer last, shown until the step is done. Drafts: the author
// rewords them in their own voice.

import type { Puzzles } from "../engine.ts";
import type { Model } from "./model.ts";

export interface HintDraft {
  /** Names the hint's ID. */
  name: string;
  /** Lua: true while the step still needs doing. */
  when: string;
  steps: string[];
}

const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
/** "Search the worn mat" → "SEARCH THE WORN MAT.", as typed. */
const typed = (s: string) => `${s.replace(/[.:]+$/, "").replace(/^\((\w+)\)/, "$1").toUpperCase()}.`;

/** The doors into a room, locked at the start. */
function lockedDoorsInto(model: Model, room: string): string[] {
  const out = new Set<string>();
  for (const r of model.roomIds()) {
    for (const e of model.exits(r)) if (e.to === room && e.door && model.door(e.door)?.locked) out.add(e.door);
  }
  return [...out];
}

/**
 * Hints for the puzzle chain's steps that can tell when they're done: the
 * steps on the way to an ending first, each after what it needs, then side
 * puzzles. HINT gives the first whose `when` is true, so that's the
 * earliest step still to do.
 */
export function draftHints(model: Model, chain: Puzzles["steps"], endings: Puzzles["endings"] = []): HintDraft[] {
  const out: HintDraft[] = [];
  const needed = new Set(endings.flatMap((e) => e.needs ?? []));
  const order = chain.map((_, i) => i).sort((a, b) => Number(!needed.has(a)) - Number(!needed.has(b)) || a - b);
  for (const step of order.map((i) => chain[i])) {
    // "Search the worn mat: a brass key." The how, and what it gives.
    const [how, what] = step.text.split(/: (?=[^:]*$)/);
    if (step.kind === "find") {
      const t = model.thing(step.id);
      if (!t) continue;
      // Done once it's found: seen, or held, whatever happens to it after.
      const when = `not world.${step.id}.seen`;
      const need = what ? cap(what.replace(/\.$/, "")) : `The ${t.name}`;
      const room = model.roomOf(step.id);
      const where = room && room !== "player" && model.room(room) ? `, in the ${model.room(room)!.name}` : "";
      // "Get the coin from the box", "Search the worn mat", "Unlock the chest with the brass key".
      const get = /^Get the (.+) from (the .+)\.$/.exec(step.text);
      const act = /^\(?(\w+)\)? (the .+?)(?: with (the .+))?$/.exec(how);
      const steps = get
        ? [`${need} might be useful. Is it inside something?`, `Look inside ${get[2]}${where}.`, `TAKE THE ${get[1].toUpperCase()} FROM ${get[2].toUpperCase()}.`]
        : act && what
          ? [
              `${need} might be useful. Where could it be?`,
              act[3] ? `${cap(act[2])} and ${act[3]} go together.` : `Take a closer look at ${act[2]}${where}.`,
              typed(how),
            ]
          : [`${need} might be useful. Where could it be?`, step.text];
      out.push({ name: `find ${t.name}`, when, steps });
    } else if (step.kind === "unlock") {
      const doors = lockedDoorsInto(model, step.id);
      if (!doors.length) continue;
      const room = model.room(step.id)?.name ?? step.id;
      const door = model.door(doors[0])?.name ?? "door";
      const code = /^Enter the code (\S+)/.exec(how);
      out.push({
        name: `open ${room}`,
        when: doors.map((d) => `world.${d}.locked`).join(" and "),
        steps: [`The ${door} is locked. What might open it?`, code ? `There's a code for it somewhere.` : `${how.replace(/^Unlock the .+? with /, "Try ")}.`, code ? `TYPE ${code[1]}.` : typed(how)],
      });
    }
  }
  return out;
}
