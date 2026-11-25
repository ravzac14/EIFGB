// The editor's panels for earning a living and word of mouth: a
// character's paid jobs and what they say once gossip reaches them, what
// busking pays in a room, and instruments to busk with.

import { slug } from "./model.ts";
import { codeBox } from "./story.ts";
import { boolField, button, check, el, field, help, numField, select, type InspectorContext } from "./ui.ts";

/** A map or list in the game file, as plain values. */
const plain = (v: unknown): unknown => ((v as { toJSON?: () => unknown } | undefined)?.toJSON?.() ?? v);

const text = (ctx: InspectorContext, path: string[], label: string, opts: { multiline?: boolean; placeholder?: string; help?: string } = {}) =>
  field(label, String(ctx.model.get(path) ?? ""), (v) => ctx.change((x) => x.set(path, v.trim() ? v : undefined), { coalesce: path.join("."), redraw: false }), opts);

/** What can reach someone by word of mouth, with how the editor says it. */
const DEEDS: [string, string][] = [
  ["robbed", "the player robbed someone"],
  ["attacked", "the player attacked someone"],
  ["fooled", "the player fooled someone"],
  ["gift", "the player gave someone a gift"],
  ["spared", "the player spared someone"],
  ["broke_promise", "the player broke a promise"],
  ["kept_promise", "the player kept a promise"],
];

/** For a character: the jobs they pay for, and what they say once they've heard what the player did. */
export function characterWork(ctx: InspectorContext, id: string): HTMLElement[] {
  const m = ctx.model;
  const p = ["characters", id];
  const jobs = Object.keys((plain(m.get([...p, "jobs"])) ?? {}) as Record<string, unknown>);
  const things = m.things();
  const out: HTMLElement[] = [el("h3", { textContent: "Work they pay for" })];
  if (!jobs.length) out.push(help("Errands they'll pay the player for, one at a time, in order: ASK THEM FOR WORK, or TALK TO them with nothing else to say. Done when the player brings what it's for (or a condition comes true), they pay, and JOURNAL lists it."));
  for (const key of jobs) {
    const j = [...p, "jobs", key];
    const wants = m.get([...j, "for"]);
    const gives = m.list([...j, "gives"]);
    const box = el("div", { class: "radio column" });
    for (const t of things.filter((t) => t.id !== wants)) box.append(check(t.name, gives.includes(t.id), (v) => ctx.change((x) => x.setList([...j, "gives"], v ? [...gives, t.id] : gives.filter((g) => g !== t.id)))));
    const done = String(m.get([...j, "done"]) ?? "");
    out.push(
      el(
        "details",
        { open: !m.get([...j, "say"]) },
        el("summary", { textContent: `Job: ${key.replace(/_/g, " ")}` }),
        ...text(ctx, [...j, "say"], "How they offer it", { multiline: true, placeholder: '"Fetch me a sack of flour from the mill? Three coins in it."' }),
        ...text(ctx, [...j, "task"], "The player's to", { placeholder: "fetch flour from the mill", help: "For the journal: “For the baker: fetch flour from the mill.”" }),
        el("label", { textContent: "It's done when the player brings" }),
        select(
          [["", "(nothing: when a condition is true)"], ...things.map((t): [string, string] => [t.id, t.name])],
          typeof wants === "string" ? wants : "",
          (v) => ctx.change((x) => {
            x.set([...j, "for"], v || undefined);
            if (v) x.set([...j, "done"], undefined);
          }),
          "It's done when the player brings",
        ),
        ...(typeof wants === "string"
          ? []
          : [
              el("label", { textContent: "Or done once this is true (Lua)" }),
              codeBox(done, (v) => ctx.change((x) => x.set([...j, "done"], v.trim() || undefined), { coalesce: `${j.join(".")}.done`, redraw: false }), "Done once", 1),
              help("Checked each turn while they can see the player, like vars.weeds_pulled, or world.goat:is_in(world.pen)."),
            ]),
        ...numField(ctx, [...j, "pays"], "They pay (money)", m.get([...j, "pays"]), "Added to the player's money."),
        el("label", { textContent: "And give the player" }),
        box,
        ...text(ctx, [...j, "thanks"], "When it's done, they say", { multiline: true, placeholder: '"Lovely. Here you are."' }),
        el("div", { class: "actions" }, button("Delete this job", () => ctx.change((x) => x.set(j, undefined)), "danger")),
      ),
    );
  }
  const name = el("input", { type: "text", placeholder: "fetch_flour", ariaLabel: "New job's name" }) as HTMLInputElement;
  out.push(
    el("div", { class: "add two" }, name, button("Add a job", () => {
      const key = slug(name.value || `job ${jobs.length + 1}`, "job");
      if (jobs.includes(key)) return name.focus();
      ctx.change((x) => x.set([...p, "jobs", key], { say: "", pays: 1 }));
    })),
  );
  // Gossip: what they say once word reaches them.
  const friends = m.list([...p, "friends"]);
  const heard = DEEDS.filter(([d]) => typeof m.get([...p, "heard", d]) === "string").length;
  out.push(
    el(
      "details",
      { open: heard > 0 },
      el("summary", { textContent: "When word reaches them" }),
      help(
        friends.length
          ? "What the player does gets round, a friend at a time: they say so the next time they see the player. Empty: “The miller gives you a hard look: word's got round that you robbed the baker.”"
          : "Give them friends (in How they behave) and word of what the player does reaches them, a friend at a time. What they say then:",
      ),
      ...DEEDS.flatMap(([d, what]) => text(ctx, [...p, "heard", d], `Heard ${what}`, { placeholder: "" })),
    ),
  );
  return out;
}

/** For a room: what busking pays here. */
export function roomBusking(ctx: InspectorContext, id: string): HTMLElement[] {
  const m = ctx.model;
  const p = ["rooms", id, "busking"];
  const v = plain(m.get(p));
  const mode = v === false ? "none" : v && typeof v === "object" ? "pays" : v === true ? "pays" : "crowd";
  const out: HTMLElement[] = [
    el("label", { textContent: "Busking here (SING, PLAY THE FIDDLE)" }),
    select(
      [
        ["crowd", "Pays when it's crowded: 1 or 2 coins, 3 times"],
        ["pays", "Pays, crowd or not"],
        ["none", "Nobody pays here"],
      ],
      mode,
      (next) => ctx.change((x) => x.set(p, next === "none" ? false : next === "pays" ? { pays: "1-2", times: 3 } : undefined)),
      "Busking here",
    ),
  ];
  if (mode === "pays" && v && typeof v === "object") {
    out.push(
      ...field("Coins each time", String((v as { pays?: unknown }).pays ?? ""), (s) => ctx.change((x) => x.set([...p, "pays"], /^\d+$/.test(s.trim()) ? Number(s.trim()) : s.trim() || undefined), { coalesce: `${id}.busk.pays`, redraw: false }), {
        placeholder: "1-2",
        help: "A number, or a range like 1-3. An instrument earns a coin more.",
      }),
      ...numField(ctx, [...p, "times"], "Times, before people have heard enough", (v as { times?: unknown }).times, "They'll pay again a while later.", "3"),
    );
  }
  return out;
}

/** For an object: a musical instrument. */
export function thingInstrument(ctx: InspectorContext, id: string): HTMLElement[] {
  return boolField(ctx, ["objects", id, "instrument"], "A musical instrument, to busk with", false, "PLAY IT where people pay a busker (in a crowd, say) and it earns a coin more than singing.");
}
