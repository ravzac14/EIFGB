// The story's moving parts: scripts on anything, endings, timers,
// conversations (choices), custom verbs, variables and script files.

import { EVENTS } from "./model.ts";
import { button, check, el, field, help, select, type InspectorContext } from "./ui.ts";

// ------------------------------------------------------------ scripts

const EVENT_LABEL: Record<string, string> = {
  start: "When the game starts",
  turn: "Every turn",
  parse_failed: "When the player types something the game doesn't understand",
  died: "When the player dies",
  enter: "When the player comes in",
  leave: "When the player leaves",
  defeated: "When they're defeated",
};

/** "before_take" → "Before TAKE", "enter" → "When the player comes in". */
export function handlerLabel(event: string, owner: "game" | "room" | "thing" | "character"): string {
  if (owner === "room" && event === "turn") return "Every turn the player is here";
  if (EVENT_LABEL[event]) return EVENT_LABEL[event];
  const m = event.match(/^(before|after)_(.+)$/);
  const verb = (m ? m[2] : event).replace(/_/g, " ").toUpperCase();
  return m ? `${m[1] === "before" ? "Before" : "After"} ${verb}` : `Instead of ${verb}`;
}

const SEED: Record<string, string> = {
  before: 'return block("Not yet.")',
  after: 'say("Something happens.")',
  instead: 'say("Something happens.")',
  event: 'say("Something happens.")',
};

/** Starter code that explains itself. */
function seed(event: string): string {
  if (event.startsWith("before_")) return SEED.before;
  if (event.startsWith("after_")) return SEED.after;
  if (event in EVENT_LABEL) return SEED.event;
  return SEED.instead;
}

/** A code box: monospace, Tab indents, grows with its text. */
export function codeBox(value: string, oninput: (v: string) => void, label: string, rows = 3): HTMLTextAreaElement {
  const ta = el("textarea", { class: "code", value, spellcheck: false, ariaLabel: label, rows: Math.max(rows, value.split("\n").length + 1) }) as HTMLTextAreaElement;
  ta.addEventListener("keydown", (e) => {
    if (e.key !== "Tab" || e.shiftKey) return;
    e.preventDefault();
    const { selectionStart: s, selectionEnd: f } = ta;
    ta.setRangeText("  ", s, f, "end");
    ta.dispatchEvent(new Event("input"));
  });
  ta.addEventListener("input", () => {
    ta.rows = Math.max(rows, ta.value.split("\n").length + 1);
    oninput(ta.value);
  });
  return ta;
}

/** Problems whose path is at or under `path`. */
function problemsAt(ctx: InspectorContext, path: string) {
  return ctx.problems.filter((d) => d.path === path || d.path.startsWith(`${path}.`) || d.path.startsWith(`${path}[`));
}

function problemNotes(ctx: InspectorContext, path: string): HTMLElement[] {
  return problemsAt(ctx, path).map((d) => el("p", { class: `problem ${d.severity}`, textContent: d.message }));
}

const API = [
  ['say("…")', "show text"],
  ['return block("…")', "in a Before script: stop the action, with a reason"],
  ["score(5)", "add to the score"],
  ['end_game("ending")', "end the game with one of your endings"],
  ['open_choice("conversation")', "start a conversation or menu"],
  ['start_timer("timer")', "start a timer (stop_timer too)"],
  ["vars.name", "one of your variables"],
  ["world.id", "any room, thing or character by ID, e.g. world.lamp.lit = true"],
  ["self, here, player", "this thing, the player's room, the player"],
  ["args.target, args.tool", "what the command was about"],
];

/**
 * The scripts on something: each with a plain-words label, its code, and
 * its problems; and a way to add another.
 */
export function scriptsSection(panel: HTMLElement, ctx: InspectorContext, owner: string[], kind: "game" | "room" | "thing" | "character") {
  const m = ctx.model;
  const base = owner.length ? `${owner.join(".")}.on` : "on";
  panel.append(el("h3", { textContent: kind === "game" ? "Game-wide scripts" : "Scripts" }));
  const list = m.handlers(owner);
  if (!list.length) panel.append(help(kind === "game" ? "Lua that runs at the start, every turn, or for any command." : "Lua that runs when the player does something with it, in place of or around what normally happens."));
  for (const h of list) {
    const box = el("div", { class: "script" });
    box.append(
      el(
        "div",
        { class: "script-head" },
        el("strong", { textContent: handlerLabel(h.event, kind) }),
        el("code", { class: "dim", textContent: h.event }),
        button("✕", () => ctx.change((x) => x.setHandler(owner, h.event, "")), "small"),
      ),
      codeBox(h.code, (v) => ctx.change((x) => x.setHandler(owner, h.event, v || " "), { coalesce: `${base}.${h.event}`, redraw: false }), handlerLabel(h.event, kind)),
      ...problemNotes(ctx, `${base}.${h.event}`),
    );
    panel.append(box);
  }

  // Add one: an event, or before / instead of / after a verb.
  const events = EVENTS[kind] as readonly string[];
  const taken = new Set(list.map((h) => h.event));
  let when = events.length ? `event` : "before";
  let what = events[0] ?? ctx.verbs[0]?.id ?? "take";
  const verbs = [...ctx.verbs.map((v) => [v.id, `${v.id.replace(/_/g, " ")}  (${v.example})`] as [string, string]), ...m.ids("verbs").map((v): [string, string] => [v, `${v} (your verb)`])];
  const whenSel = select(
    [...(events.length ? ([["event", "When…"]] as [string, string][]) : []), ["before", "Before"], ["instead", "Instead of"], ["after", "After"]],
    when,
    (v) => {
      when = v;
      whatSel.replaceChildren(...options(v === "event" ? events.map((e): [string, string] => [e, EVENT_LABEL[e]?.replace(/^When /, "") ?? e]) : verbs, (what = v === "event" ? events[0] : verbs[0][0])));
    },
    "When",
  );
  const whatSel = select(when === "event" ? events.map((e): [string, string] => [e, EVENT_LABEL[e]?.replace(/^When /, "") ?? e]) : verbs, what, (v) => (what = v), "What");
  const add = button("Add a script", () => {
    const key = when === "event" ? what : when === "instead" ? what : `${when}_${what}`;
    if (taken.has(key)) return ctx.notice("There's already a script for that: change that one.");
    ctx.change((x) => x.setHandler(owner, key, seed(key)));
  });
  panel.append(el("div", { class: "add three" }, whenSel, whatSel, add));
  panel.append(
    el(
      "details",
      {},
      el("summary", { textContent: "What can a script do?" }),
      el("ul", { class: "api" }, ...API.map(([c, d]) => el("li", {}, el("code", { textContent: c }), ` ${d}`))),
      help("Scripts are Lua. The scripting tutorial in the Guide teaches it from the start."),
    ),
  );
}

function options(list: [string, string][], value: string): HTMLOptionElement[] {
  return list.map(([v, t]) => {
    const o = document.createElement("option");
    o.value = v;
    o.textContent = t;
    o.selected = v === value;
    return o;
  });
}

// ---------------------------------------------- the game panel's story list

function entryRow(ctx: InspectorContext, kind: "ending" | "timer" | "hint" | "choice" | "verb", id: string, note: string) {
  const selected = ctx.selection?.kind === kind && ctx.selection.id === id;
  const row = el("div", { class: `thing${selected ? " selected" : ""}` }, el("span", { class: "name", textContent: id.replace(/_/g, " ") }), el("span", { class: "tag", textContent: note }));
  if (problemsAt(ctx, `${kind === "choice" ? "choices" : `${kind}s`}.${id}`).some((d) => d.severity === "error")) row.classList.add("bad");
  row.addEventListener("click", () => ctx.select({ kind, id }));
  return el("li", {}, row);
}

/** Endings, timers, conversations, verbs, variables and script files, for the game panel. */
export function storySections(panel: HTMLElement, ctx: InspectorContext) {
  const m = ctx.model;
  const group = (title: string, ...kids: (Node | string)[]) => panel.append(el("details", { class: "group", open: true }, el("summary", { textContent: title }), ...kids));

  // Endings.
  const endings = m.ids("endings");
  const endingName = el("input", { type: "text", placeholder: "escaped, eaten by wolves…", ariaLabel: "New ending's name" }) as HTMLInputElement;
  group(
    "Endings",
    el("ul", { class: "things" }, ...endings.map((e) => entryRow(ctx, "ending", e, String(m.get(["endings", e, "kind"]) ?? "neutral")))),
    ...(endings.length ? [] : [help("How the game can end. Without any, it never does (unless the player dies).")]),
    el(
      "div",
      { class: "add two" },
      endingName,
      button("Add an ending", () => {
        let id = "";
        ctx.change((x) => (id = x.addEnding(endingName.value || "the end")));
        ctx.select({ kind: "ending", id });
      }),
    ),
  );

  // Timers.
  const timers = m.ids("timers");
  group(
    "Timers",
    el(
      "ul",
      { class: "things" },
      ...timers.map((t) => {
        const after = m.get(["timers", t, "after"]);
        const every = m.get(["timers", t, "every"]);
        return entryRow(ctx, "timer", t, every ? `every ${every} turns` : `after ${after ?? "?"} turns`);
      }),
    ),
    ...(timers.length ? [] : [help("Things that happen after a number of turns: a storm, a train leaving, a pie cooling.")]),
    el("div", { class: "actions" }, button("Add a timer", () => {
      let id = "";
      ctx.change((x) => (id = x.addTimer()));
      ctx.select({ kind: "timer", id });
    })),
  );

  // Hints.
  const hints = m.ids("hints");
  group(
    "Hints",
    el("ul", { class: "things" }, ...hints.map((h) => entryRow(ctx, "hint", h, `${m.list(["hints", h, "steps"]).length} steps`))),
    ...(hints.length
      ? [help("HINT gives the next step of the first one here that's offered, so keep them in the order players meet the puzzles.")]
      : [help("Help for players who are stuck: for each puzzle, a nudge first and the answer last. Players type HINT.")]),
    el("div", { class: "actions" }, button("Add a hint", () => {
      let id = "";
      ctx.change((x) => (id = x.addHint()));
      ctx.select({ kind: "hint", id });
    })),
  );

  // Conversations and menus.
  const choices = m.ids("choices");
  const choiceName = el("input", { type: "text", placeholder: "talk with Meg, the radio…", ariaLabel: "New conversation's name" }) as HTMLInputElement;
  group(
    "Conversations and menus",
    el(
      "ul",
      { class: "things" },
      ...choices.map((c) => {
        const who = m.characters().filter((x) => x.talk === c).map((x) => x.name);
        return entryRow(ctx, "choice", c, who.length ? `with ${who.join(", ")}` : `${m.choice(c)?.nodes.length ?? 0} parts`);
      }),
    ),
    ...(choices.length ? [] : [help("Conversations with characters, or menus like a radio's dial: the player picks from numbered options.")]),
    el(
      "div",
      { class: "add two" },
      choiceName,
      button("Add a conversation", () => {
        let id = "";
        ctx.change((x) => (id = x.addChoice(choiceName.value || "conversation")));
        ctx.select({ kind: "choice", id });
      }),
    ),
  );

  // Custom verbs.
  const verbs = m.ids("verbs");
  const verbName = el("input", { type: "text", placeholder: "ring, dance, pray…", ariaLabel: "New verb" }) as HTMLInputElement;
  group(
    "Your own verbs",
    el("ul", { class: "things" }, ...verbs.map((v) => entryRow(ctx, "verb", v, m.list(["verbs", v, "patterns"])[0] ?? ""))),
    ...(verbs.length ? [] : [help("Commands the standard ones don't cover: RING THE BELL, PRAY. Give things a script for them.")]),
    el(
      "div",
      { class: "add two" },
      verbName,
      button("Add a verb", () => {
        if (!verbName.value.trim()) return verbName.focus();
        let id = "";
        ctx.change((x) => (id = x.addVerb(verbName.value)));
        ctx.select({ kind: "verb", id });
      }),
    ),
  );

  // Variables.
  group("Variables", ...variablesSection(ctx));

  // Script files.
  const files = m.list(["scripts"]);
  const fileName = el("input", { type: "text", placeholder: "helpers", ariaLabel: "New script file's name" }) as HTMLInputElement;
  group(
    "Script files",
    el(
      "ul",
      { class: "things" },
      ...files.map((f) => {
        const row = el("div", { class: "thing" }, el("span", { class: "name", textContent: f }), el("span", { class: "tag", textContent: "open" }));
        row.addEventListener("click", () => ctx.openFile(f));
        return el("li", {}, row);
      }),
    ),
    ...(files.length ? [] : [help("Lua files for functions your scripts share.")]),
    el(
      "div",
      { class: "add two" },
      fileName,
      button("Add a file", () => {
        const path = ctx.addScriptFile(fileName.value || "helpers");
        if (path) ctx.openFile(path);
      }),
    ),
  );
}

/** Values as typed: true/false and numbers become those; anything else is text. */
export function parseValue(s: string): unknown {
  const t = s.trim();
  if (t === "true" || t === "false") return t === "true";
  if (/^-?\d+(\.\d+)?$/.test(t)) return Number(t);
  return s;
}

function variablesSection(ctx: InspectorContext): HTMLElement[] {
  const m = ctx.model;
  const rows = m.ids("vars").map((v) => {
    const raw = m.get(["vars", v]);
    const simple = raw === null || ["string", "number", "boolean"].includes(typeof raw);
    const input = el("input", { type: "text", value: simple ? String(raw ?? "") : "(a list or table)", disabled: !simple, ariaLabel: `${v}'s starting value` }) as HTMLInputElement;
    input.addEventListener("input", () => ctx.change((x) => x.set(["vars", v], parseValue(input.value) ?? ""), { coalesce: `vars.${v}`, redraw: false }));
    return el(
      "div",
      { class: "var" },
      el("code", { textContent: `vars.${v}` }),
      input,
      button("✕", () => {
        if (confirm(`Remove the variable ${v}? Scripts that use it will stop working.`)) ctx.change((x) => x.deleteEntry("vars", v));
      }, "small"),
    );
  });
  const name = el("input", { type: "text", placeholder: "name", ariaLabel: "New variable's name" }) as HTMLInputElement;
  const value = el("input", { type: "text", placeholder: "0, true, or text", ariaLabel: "Its starting value" }) as HTMLInputElement;
  return [
    ...(rows.length ? rows : [help("Story state your scripts keep: vars.gold, vars.met_the_king. Saved and undone with the game.")]),
    el(
      "div",
      { class: "add three" },
      name,
      value,
      button("Add", () => {
        if (!name.value.trim()) return name.focus();
        ctx.change((x) => x.addVar(name.value, parseValue(value.value || "0")));
      }),
    ),
  ];
}

// -------------------------------------------------------------- endings

function renameRow(ctx: InspectorContext, section: string, id: string, kind: "ending" | "timer" | "hint" | "choice" | "verb") {
  const input = el("input", { type: "text", value: id, ariaLabel: "ID" }) as HTMLInputElement;
  const msg = help("");
  return [
    el("label", { textContent: "ID" }),
    el(
      "div",
      { class: "row" },
      input,
      button("Rename", () => {
        let next: string | null = null;
        ctx.change((x) => (next = x.renameEntry(section, id, input.value)));
        if (next) ctx.select({ kind, id: next });
        else msg.textContent = "That name is taken.";
      }),
    ),
    msg,
  ];
}

export function endingPanel(panel: HTMLElement, ctx: InspectorContext, id: string) {
  const m = ctx.model;
  const p = ["endings", id];
  const kind = String(m.get([...p, "kind"]) ?? "neutral");
  const radio = (v: string, text: string) => {
    const r = el("input", { type: "radio", name: `ending-${id}`, value: v, checked: kind === v });
    r.addEventListener("change", () => ctx.change((x) => x.set([...p, "kind"], v)));
    return el("label", {}, r, text);
  };
  const usedBy = m.ids("timers").filter((t) => m.get(["timers", t, "end"]) === id);
  panel.append(
    el("p", { class: "kind", textContent: "Ending" }),
    el("h2", { textContent: id.replace(/_/g, " ") }),
    el("label", { textContent: "The player has" }),
    el("div", { class: "radio" }, radio("win", "Won"), radio("loss", "Lost"), radio("neutral", "Neither")),
    ...field("What they're told", String(m.get([...p, "text"]) ?? ""), (v) => ctx.change((x) => x.set([...p, "text"], v || " "), { coalesce: `${id}.text`, redraw: false }), {
      multiline: true,
    }),
    ...problemNotes(ctx, `endings.${id}`),
    el("h3", { textContent: "How it's reached" }),
    ...(usedBy.length ? usedBy.map((t) => el("p", {}, "When the timer ", button(t.replace(/_/g, " "), () => ctx.select({ kind: "timer", id: t }), "link"), " runs out.")) : []),
    help(`From a script: end_game("${id}"). Timers can end with it too.`),
    ...renameRow(ctx, "endings", id, "ending"),
    help("Renaming updates timers, but not Lua scripts."),
    el("div", { class: "actions" }, button("Delete this ending", () => {
      if (!confirm(`Delete the ending “${id}”?`)) return;
      ctx.change((x) => x.deleteEntry("endings", id));
      ctx.select(null);
    }, "danger")),
  );
}

// --------------------------------------------------------------- timers

/** A rule like `search mat`: an action and a thing, for started_by / stopped_by. */
function ruleBuilder(ctx: InspectorContext, path: string[], label: string, none: string) {
  const m = ctx.model;
  const rules = m.list(path);
  if (rules.length > 1 || (rules[0] && rules[0].trim().split(/\s+(?![^(]*\))/).length !== 2)) {
    return [el("p", { class: "note", textContent: `${label}: ${rules.join("; or ")}. Change it in the game file.` })];
  }
  const [verb0, what0] = (rules[0] ?? "").trim().split(/\s+(?![^(]*\))/);
  let verb = verb0 || "";
  let what = what0 || "";
  const write = () => ctx.change((x) => x.set(path, verb && what ? `${verb} ${what}` : undefined));
  const targets = [...m.things().map((t): [string, string] => [t.id, `the ${t.name}`]), ...m.characters().map((c): [string, string] => [c.id, c.name])];
  return [
    el("label", { textContent: label }),
    el(
      "div",
      { class: "add two" },
      select([["", none], ...ctx.verbs.map((v): [string, string] => [v.id, v.id.replace(/_/g, " ")]), ["(physical action)", "touches or moves"]], verb, (v) => {
        verb = v;
        write();
      }, `${label}: action`),
      select([["", "choose…"], ...targets], what, (v) => {
        what = v;
        write();
      }, `${label}: thing`),
    ),
  ];
}

export function timerPanel(panel: HTMLElement, ctx: InspectorContext, id: string) {
  const m = ctx.model;
  const p = ["timers", id];
  const every = m.get([...p, "every"]);
  const repeat = typeof every === "number";
  const count = (repeat ? every : m.get([...p, "after"])) as number | undefined;
  const n = el("input", { type: "text", inputMode: "numeric", value: count === undefined ? "" : String(count), ariaLabel: "Turns" }) as HTMLInputElement;
  const how = select([["after", "Once, after"], ["every", "Every"]], repeat ? "every" : "after", (v) =>
    ctx.change((x) => {
      const k = Number.parseInt(n.value, 10) || 5;
      x.set([...p, v === "every" ? "after" : "every"], undefined);
      x.set([...p, v], k);
    }), "How often");
  n.addEventListener("input", () => {
    const k = Number.parseInt(n.value, 10);
    if (k >= 1) ctx.change((x) => x.set([...p, repeat ? "every" : "after"], k), { coalesce: `${id}.n`, redraw: false });
  });
  const endings = m.ids("endings");
  const str = (k: string) => String(m.get([...p, k]) ?? "");
  panel.append(
    el("p", { class: "kind", textContent: "Timer" }),
    el("h2", { textContent: id.replace(/_/g, " ") }),
    el("label", { textContent: "It goes off" }),
    el("div", { class: "add three" }, how, n, el("span", { class: "dim", textContent: "turns" })),
    help("Turns are commands that take time: LOOK and SAVE don't count."),
    ...ruleBuilder(ctx, [...p, "started_by"], "It starts when the player", "(when the game starts)"),
    ...ruleBuilder(ctx, [...p, "stopped_by"], "It stops when the player", "(never)"),
    el("h3", { textContent: "When it goes off" }),
    ...field("It says", str("say"), (v) => ctx.change((x) => x.set([...p, "say"], v || undefined), { coalesce: `${id}.say`, redraw: false }), { multiline: true, placeholder: "Thunder rolls closer." }),
    el("label", { textContent: "Then ends the game with" }),
    select([["", "(nothing: the game goes on)"], ...endings.map((e): [string, string] => [e, e.replace(/_/g, " ")])], str("end"), (v) => ctx.change((x) => x.set([...p, "end"], v || undefined)), "Ending"),
    el(
      "details",
      { open: !!(str("when") || str("do")) },
      el("summary", { textContent: "With a script" }),
      el("label", { textContent: "Only if (Lua)" }),
      codeBox(str("when"), (v) => ctx.change((x) => x.set([...p, "when"], v.trim() || undefined), { coalesce: `${id}.when`, redraw: false }), "Only if", 1),
      help("A condition, like not vars.ship_saved. If it's false, the timer does nothing."),
      el("label", { textContent: "Runs (Lua)" }),
      codeBox(str("do"), (v) => ctx.change((x) => x.set([...p, "do"], v.trim() ? v : undefined), { coalesce: `${id}.do`, redraw: false }), "Runs"),
    ),
    ...problemNotes(ctx, `timers.${id}`),
    ...renameRow(ctx, "timers", id, "timer"),
    help(`Scripts can start and stop it: start_timer("${id}"), stop_timer("${id}").`),
    el("div", { class: "actions" }, button("Delete this timer", () => {
      if (!confirm(`Delete the timer “${id}”?`)) return;
      ctx.change((x) => x.deleteEntry("timers", id));
      ctx.select(null);
    }, "danger")),
  );
}

// ---------------------------------------------------------------- hints

export function hintPanel(panel: HTMLElement, ctx: InspectorContext, id: string) {
  const m = ctx.model;
  const p = ["hints", id];
  const all = m.ids("hints");
  const at = all.indexOf(id);
  const steps = m.list([...p, "steps"]);
  const when = String(m.get([...p, "when"]) ?? "");
  const stepRow = (text: string, i: number) => {
    const box = el("textarea", { rows: 2, value: text, ariaLabel: `Step ${i + 1}`, placeholder: i === 0 ? "A nudge…" : "…or the answer" }) as HTMLTextAreaElement;
    box.addEventListener("input", () =>
      ctx.change(
        (x) => {
          const now = x.list([...p, "steps"]);
          now[i] = box.value;
          x.setHintSteps(id, now);
        },
        { coalesce: `${id}.step${i}`, redraw: false },
      ),
    );
    const remove = button("Remove", () => ctx.change((x) => x.setHintSteps(id, x.list([...p, "steps"]).filter((_, k) => k !== i))));
    remove.disabled = steps.length < 2;
    remove.title = steps.length < 2 ? "A hint needs at least one step" : `Remove step ${i + 1}`;
    return el("li", { class: "hint-step" }, el("span", { class: "dim", textContent: `${i + 1}.` }), box, remove);
  };
  panel.append(
    el("p", { class: "kind", textContent: "Hint" }),
    el("h2", { textContent: id.replace(/_/g, " ") }),
    help("When a player types HINT, they get the next step: the first, then the second, and so on, with the last one repeated. Start with a nudge, and end with the answer."),
    el("label", { textContent: "Steps" }),
    el("ol", { class: "hint-steps" }, ...steps.map(stepRow)),
    el("div", { class: "actions" }, button("Add a step", () => ctx.change((x) => x.setHintSteps(id, [...x.list([...p, "steps"]), ""])))),
    el("label", { textContent: "Offered while (Lua)" }),
    codeBox(when, (v) => ctx.change((x) => x.set([...p, "when"], v.trim() || undefined), { coalesce: `${id}.when`, redraw: false }), "Offered while", 1),
    help("A condition that's true from when the puzzle can be found until it's solved, like world.small_key.hidden or not world.gate.open. Empty: always offered."),
    el("label", { textContent: "Order" }),
    el(
      "div",
      { class: "row" },
      Object.assign(button("Earlier", () => ctx.change((x) => x.moveEntry("hints", id, -1))), { disabled: at <= 0 }),
      Object.assign(button("Later", () => ctx.change((x) => x.moveEntry("hints", id, 1))), { disabled: at < 0 || at >= all.length - 1 }),
      el("span", { class: "dim", textContent: `${at + 1} of ${all.length}` }),
    ),
    help("HINT uses the first hint in the list that's offered, so put them in the order players meet the puzzles."),
    ...problemNotes(ctx, `hints.${id}`),
    ...renameRow(ctx, "hints", id, "hint"),
    el("div", { class: "actions" }, button("Delete this hint", () => {
      if (!confirm(`Delete the hint “${id}”?`)) return;
      ctx.change((x) => x.deleteEntry("hints", id));
      ctx.select(null);
    }, "danger")),
  );
}

// -------------------------------------------------------- conversations

export function choicePanel(panel: HTMLElement, ctx: InspectorContext, id: string) {
  const m = ctx.model;
  const c = m.choice(id)!;
  const who = m.characters().filter((x) => x.talk === id);
  const nodeIds = c.nodes.map((n) => n.id);
  panel.append(
    el("p", { class: "kind", textContent: "Conversation or menu" }),
    el("h2", { textContent: id.replace(/_/g, " ") }),
    who.length
      ? el("p", {}, "TALK TO ", ...who.map((w) => button(w.name, () => ctx.select({ kind: "character", id: w.id }), "link")), " starts it.")
      : help(`Nobody starts it yet. Choose it under Talking on a character, or start it from a script: open_choice("${id}").`),
    el("label", { textContent: "It begins with" }),
    select(c.nodes.map((n): [string, string] => [n.id, n.id.replace(/_/g, " ")]), c.start, (v) => ctx.change((x) => x.set(["choices", id, "start"], v)), "First part"),
    help("Each part is something said (or shown), and the player's replies to pick from. A reply can lead to another part, or end it."),
    ...problemNotes(ctx, `choices.${id}`).slice(0, 3),
  );

  for (const n of c.nodes) {
    const p = ["choices", id, "nodes", n.id];
    const card = el("section", { class: `node${n.id === c.start ? " start" : ""}` });
    const nameInput = el("input", { type: "text", value: n.id, class: "node-name", ariaLabel: "Part's name" }) as HTMLInputElement;
    nameInput.addEventListener("change", () => {
      let next: string | null = null;
      ctx.change((x) => (next = x.renameNode(id, n.id, nameInput.value)));
      if (!next) ctx.notice("Another part already has that name.");
    });
    card.append(
      el("div", { class: "node-head" }, nameInput, ...(n.id === c.start ? [el("span", { class: "tag", textContent: "first" })] : []), button("✕", () => {
        if (c.nodes.length < 2) return ctx.notice("A conversation needs at least one part.");
        if (confirm(`Delete the part “${n.id}”? Replies that lead to it will end the conversation instead.`)) ctx.change((x) => x.deleteNode(id, n.id));
      }, "small")),
      ...field("They say", n.text, (v) => ctx.change((x) => x.set([...p, "text"], v || undefined), { coalesce: `${id}.${n.id}.text`, redraw: false }), {
        multiline: true,
        placeholder: '"Another storm," the old man murmurs.',
      }),
    );
    const ol = el("ol", { class: "options" });
    n.options.forEach((o, i) => {
      const op = [...p, "options", i];
      const next = o.end ? "__end" : o.goto || "__stay";
      const nextSel = select(
        [...nodeIds.map((x): [string, string] => [x, `→ ${x.replace(/_/g, " ")}`]), ["__end", "→ the end"], ["__stay", "→ back to this part"]],
        next,
        (v) => ctx.change((x) => x.setOptionNext(id, n.id, i, v === "__end" ? { end: true } : v === "__stay" ? {} : { goto: v })),
        "Then",
      );
      const text = el("input", { type: "text", value: o.text, placeholder: "Ask about the lamp", ariaLabel: "Reply" }) as HTMLInputElement;
      text.addEventListener("input", () => ctx.change((x) => x.set([...op, "text"], text.value || " "), { coalesce: `${id}.${n.id}.${i}`, redraw: false }));
      ol.append(
        el(
          "li",
          {},
          el("div", { class: "option" }, text, nextSel, button("✕", () => ctx.change((x) => x.removeOption(id, n.id, i)), "small")),
          el(
            "details",
            { open: !!(o.when || o.say || o.do) },
            el("summary", { textContent: "More" }),
            ...field("Also says", o.say, (v) => ctx.change((x) => x.set([...op, "say"], v || undefined), { coalesce: `${id}.${n.id}.${i}.say`, redraw: false })),
            el("label", { textContent: "Only shown if (Lua)" }),
            codeBox(o.when, (v) => ctx.change((x) => x.set([...op, "when"], v.trim() || undefined), { coalesce: `${id}.${n.id}.${i}.when`, redraw: false }), "Only shown if", 1),
            el("label", { textContent: "Runs (Lua)" }),
            codeBox(o.do, (v) => ctx.change((x) => x.set([...op, "do"], v.trim() ? v : undefined), { coalesce: `${id}.${n.id}.${i}.do`, redraw: false }), "Runs", 1),
          ),
        ),
      );
    });
    card.append(el("label", { textContent: "The player can reply" }), ol, el("div", { class: "actions" }, button("+ Add a reply", () => ctx.change((x) => x.addOption(id, n.id)))));
    if (!n.options.length) {
      card.append(
        help("No replies: it shows the text and moves on."),
        el("label", { textContent: "Then goes to" }),
        select([["", "(the end)"], ...nodeIds.filter((x) => x !== n.id).map((x): [string, string] => [x, x.replace(/_/g, " ")])], n.goto, (v) => ctx.change((x) => x.set([...p, "goto"], v || undefined)), "Then goes to"),
      );
    }
    card.append(...problemNotes(ctx, `choices.${id}.nodes.${n.id}`));
    panel.append(card);
  }
  panel.append(
    el("div", { class: "actions" }, button("+ Add a part", () => ctx.change((x) => x.addNode(id)))),
    ...renameRow(ctx, "choices", id, "choice"),
    help("Renaming updates the characters who use it, but not Lua scripts."),
    el("div", { class: "actions" }, button("Delete this conversation", () => {
      if (!confirm(`Delete the conversation “${id}”?`)) return;
      ctx.change((x) => x.deleteEntry("choices", id));
      ctx.select(null);
    }, "danger")),
  );
}

// ---------------------------------------------------------------- verbs

export function verbPanel(panel: HTMLElement, ctx: InspectorContext, id: string) {
  const m = ctx.model;
  const p = ["verbs", id];
  const patterns = m.list([...p, "patterns"]);
  const users = [...m.objectIds().map((o) => ["objects", o]), ...m.characterIds().map((c) => ["characters", c]), ...m.roomIds().map((r) => ["rooms", r])].filter((o) =>
    m.handlers(o).some((h) => h.event === id || h.event === `before_${id}` || h.event === `after_${id}`),
  );
  const pat = codeBox(patterns.join("\n"), (v) => ctx.change((x) => x.setPatterns(id, v.split("\n")), { coalesce: `${id}.patterns`, redraw: false }), "Patterns", 2);
  panel.append(
    el("p", { class: "kind", textContent: "Verb" }),
    el("h2", { textContent: id.replace(/_/g, " ") }),
    el("label", { textContent: "What players type, one per line" }),
    pat,
    help("{target} is the thing it's done to, {tool} what it's done with, {text} any words: ring {target}, ring {target} with {tool}."),
    ...field("Said when nothing has a script for it", String(m.get([...p, "default"]) ?? ""), (v) => ctx.change((x) => x.set([...p, "default"], v || undefined), { coalesce: `${id}.default`, redraw: false }), {
      placeholder: "Nothing happens.",
    }),
    check("The player has to be able to reach it", m.bool([...p, "reach"], true), (v) => ctx.change((x) => x.setBool([...p, "reach"], v, true))),
    ...problemNotes(ctx, `verbs.${id}`),
    el("h3", { textContent: "What responds to it" }),
    ...(users.length
      ? users.map((o) => {
          const kind = o[0] === "objects" ? "thing" : o[0] === "characters" ? "character" : "room";
          const name = (kind === "room" ? m.room(o[1])?.name : kind === "character" ? m.character(o[1])?.name : m.thing(o[1])?.name) ?? o[1];
          return el("p", {}, button(name, () => ctx.select({ kind, id: o[1] } as never), "link"));
        })
      : [help(`Nothing yet. Give a thing a script for it: “Instead of ${id.toUpperCase()}”.`)]),
    ...renameRow(ctx, "verbs", id, "verb"),
    help("Renaming doesn't rename the scripts for it."),
    el("div", { class: "actions" }, button("Delete this verb", () => {
      if (!confirm(`Delete the verb “${id}”?`)) return;
      ctx.change((x) => x.deleteEntry("verbs", id));
      ctx.select(null);
    }, "danger")),
  );
}
