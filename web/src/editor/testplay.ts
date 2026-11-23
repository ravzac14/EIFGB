// Test play: the game, played in the editor's drawer, so trying something
// doesn't mean switching tabs. It plays the game as it is when started (or
// started over), from the start room or from the room selected on the map,
// and shows on the map which room the player is in.
//
// It can also start part-way: at any step of the walkthrough, or, after an
// edit, where the game in progress had got to, by playing the moves again
// on the new version without showing them.
//
// What's typed in a game played from the start can be saved as the
// game's walkthrough.txt; the editor's check replays it after every edit
// and says here (and under Problems) when it no longer wins.

import type { Engine, Files, Missed, Output, Walkthrough, Wandered } from "../engine.ts";
import { Model } from "./model.ts";
import { figure, pictureSrc } from "../pictures.ts";

/** The walkthrough's file, as in the example games and starter templates. */
export const WALKTHROUGH = "walkthrough.txt";
/** Test play always uses this seed, as the walkthrough check does, so a recording replays the same. */
const SEED = 1;

export interface TestPlayHost {
  engine(): Promise<Engine>;
  /** The project's files as they are now. */
  files(): Files;
  /** The room selected on the map, if a room is selected. */
  selectedRoom(): { id: string; name: string } | null;
  /** Marks the room the player is in on the map (null: none). */
  markHere(room: string | null): void;
  /** Opens the Problems tab. */
  showProblems(): void;
  /** Writes walkthrough.txt, or another walkthrough file. */
  saveWalkthrough(text: string, file?: string): void;
  /** A room's, thing's or character's name, by its ID. */
  nameOf(id: string): string;
  /** Selects what an ID is (a room, thing, character), or a conversation's option. */
  show(what: { id: string } | { choice: string }): void;
  /** What's in a room, to add a word to. */
  thingsIn(room: string): { id: string; name: string; section: "objects" | "characters" }[];
  /** Words a new word could mean: verbs the game knows. */
  verbWords(): string[];
  /** Fixes for what wasn't understood. */
  addSynonym(section: "objects" | "characters", id: string, word: string): void;
  addAlias(word: string, means: string): void;
  addVerb(word: string): void;
  addScenery(room: string, word: string): void;
}

/** The commands in a walkthrough, one a line, without comments, blank lines, or what a line expects (`take key => Taken.`). */
export function walkthroughCommands(text: string | undefined): string[] {
  return (text ?? "")
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l && !l.startsWith("#"))
    .map((l) => l.split("=>")[0].trim())
    .filter(Boolean);
}

/** A walkthrough for an ending: `walkthrough-feast.txt`. */
export const endingWalkthrough = (ending: string) => `walkthrough-${ending}.txt`;

/** How many moves the wanderer makes each time. */
const WANDER_MOVES = 2000;

/** A command test play didn't understand, kept until it's fixed or dismissed. */
interface Miss {
  command: string;
  room: string | null;
  roomName: string;
  missed: Missed;
}

function el<K extends keyof HTMLElementTagNameMap>(tag: K, props: Record<string, unknown> = {}, ...kids: (Node | string)[]): HTMLElementTagNameMap[K] {
  const e = Object.assign(document.createElement(tag), props);
  e.append(...kids);
  return e;
}

export class TestPlay {
  private log: HTMLElement;
  private line: HTMLInputElement;
  private form: HTMLFormElement;
  private fromRoom: HTMLButtonElement;
  private status: HTMLElement;
  private stale: HTMLElement;
  private engine: Engine | null = null;
  private running = false;
  private here: string | null = null;
  private history: string[] = [];
  private back = 0;
  /** Whether the panel is showing: the map only marks the player's room while it is. */
  private visible = false;
  /** The files the game in progress was started from. */
  private startedWith: Files | null = null;
  /** The commands typed in this game, and whether it began at the start room (only then is it a walkthrough). */
  private played: string[] = [];
  private fromStart = false;
  private won = false;
  private record: HTMLButtonElement;
  private recordEnding: HTMLButtonElement;
  /** The ending the game in progress reached, if it has. */
  private ending: string | null = null;
  private jump: HTMLSelectElement;
  /** The other walkthroughs, and what none of them reach. */
  private walks: HTMLElement;
  private wanderBox: HTMLDetailsElement;
  private wanderRuns = 0;
  private walk: HTMLElement;
  /** What wasn't understood, this session. */
  private misses: Miss[] = [];
  private missBox: HTMLDetailsElement;

  private host: TestPlayHost;

  constructor(panel: HTMLElement, host: TestPlayHost) {
    this.host = host;
    const restart = el("button", { type: "button", textContent: "Start over" });
    restart.addEventListener("click", () => void this.start());
    this.fromRoom = el("button", { type: "button" });
    this.fromRoom.addEventListener("click", () => {
      const r = this.host.selectedRoom();
      if (r) void this.start(r.id);
    });
    this.record = el("button", { type: "button", textContent: "Save as walkthrough" });
    this.record.addEventListener("click", () => this.saveWalkthrough());
    this.recordEnding = el("button", { type: "button", hidden: true });
    this.recordEnding.addEventListener("click", () => this.ending && this.saveWalkthrough(endingWalkthrough(this.ending)));
    const wanderButton = el("button", { type: "button", textContent: "Wander" });
    wanderButton.title = `A test player tries ${WANDER_MOVES} random moves, apart from your game, and reports script errors, endings, and places it got stuck`;
    wanderButton.addEventListener("click", () => void this.wander());
    this.walks = el("div", { className: "tp-walks" });
    this.wanderBox = el("details", { className: "tp-wander", hidden: true });
    this.jump = el("select", { className: "tp-jump", hidden: true });
    this.jump.setAttribute("aria-label", "Start at a step of the walkthrough");
    this.jump.addEventListener("change", () => {
      const n = Number(this.jump.value);
      this.jump.value = "";
      if (n) void this.replay(walkthroughCommands(this.host.files()[WALKTHROUGH]).slice(0, n), "walkthrough");
    });
    this.walk = el("p", { className: "tp-walk dim" });
    this.status = el("span", { className: "dim tp-status", role: "status" });
    this.stale = el("p", { className: "tp-stale", hidden: true });
    const again = el("button", { type: "button", className: "link", textContent: "Start over" });
    again.addEventListener("click", () => void this.start());
    const replay = el("button", { type: "button", className: "link", textContent: "play the new version up to here" });
    replay.addEventListener("click", () => {
      if (this.fromStart) void this.replay([...this.played], "moves");
      else void this.start();
    });
    this.stale.append("You've changed the game since this started. ", again, " to play the new version, or ", replay, " (your moves, played again for you).");
    this.log = el("div", { className: "tp-log", ariaLive: "polite" });
    this.line = el("input", { type: "text", autocomplete: "off", spellcheck: false, ariaLabel: "Command" });
    this.line.setAttribute("autocapitalize", "off");
    this.form = el("form", { className: "tp-prompt", hidden: true }, el("span", { className: "caret", textContent: ">" }), this.line);
    this.form.addEventListener("submit", (e) => {
      e.preventDefault();
      const text = this.line.value.trim();
      this.line.value = "";
      if (text) {
        this.history.push(text);
        this.back = 0;
      }
      this.send(text);
    });
    this.line.addEventListener("keydown", (e) => {
      if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;
      e.preventDefault();
      this.back = Math.max(0, Math.min(this.history.length, this.back + (e.key === "ArrowUp" ? 1 : -1)));
      this.line.value = this.back ? this.history[this.history.length - this.back] : "";
    });
    this.log.addEventListener("click", () => {
      if (!getSelection()?.toString() && !this.form.hidden) this.line.focus({ preventScroll: true });
    });
    this.log.append(el("p", { className: "dim", textContent: "Play your game here while you work on it. Press Start over to begin." }));
    this.missBox = el("details", { className: "tp-misses", hidden: true });
    panel.append(el("div", { className: "tp-bar" }, restart, this.fromRoom, this.jump, this.record, this.recordEnding, wanderButton, this.status), this.walk, this.walks, this.wanderBox, this.missBox, this.stale, this.log, this.form);
    this.walkthrough("none");
    this.update();
  }

  /** The panel was shown or hidden. */
  shown(visible: boolean) {
    this.visible = visible;
    this.host.markHere(visible ? this.here : null);
    if (visible && !this.running) void this.start();
    else if (visible) this.line.focus({ preventScroll: true });
  }

  /** The selection changed: the "start from" button follows it. */
  update() {
    const r = this.host.selectedRoom();
    this.fromRoom.textContent = r ? `Start in ${r.name}` : "Start in a room…";
    this.fromRoom.disabled = !r;
    this.fromRoom.title = r ? `Play from ${r.name}, as if the game started there` : "Select a room on the map to start there";
    this.record.disabled = !this.fromStart || !this.played.length;
    this.recordEnding.hidden = !this.fromStart || !this.ending;
    if (this.ending) {
      this.recordEnding.textContent = `Save for the ${this.ending} ending`;
      this.recordEnding.title = `Save these ${this.played.length} commands as ${endingWalkthrough(this.ending)}, checked after every change to still reach the ${this.ending} ending`;
    }
    this.record.title = !this.fromStart
      ? "Only a game played from the start can be a walkthrough"
      : `Save the ${this.played.length} commands you've typed as ${WALKTHROUGH}, to check after every change that the game can still be won`;
  }

  /** The steps of the walkthrough to start at. */
  private fillJump(text: string | undefined) {
    const cmds = walkthroughCommands(text);
    this.jump.hidden = !cmds.length;
    this.jump.replaceChildren(el("option", { value: "", textContent: "Start at a step…" }));
    cmds.forEach((c, i) => this.jump.append(el("option", { value: String(i + 1), textContent: `After ${i + 1}. ${c}` })));
    this.jump.title = "Play the walkthrough up to a step without showing it, then carry on from there";
  }

  /** How the saved walkthrough plays now: "none" when there isn't one, "unchecked" when the game can't be played. */
  walkthrough(w: Walkthrough | "none" | "unchecked", text?: string) {
    this.fillJump(text);
    this.walk.replaceChildren();
    this.walk.className = "tp-walk dim";
    if (w === "none") {
      this.walk.textContent = `No walkthrough yet. Play to a win from the start, then Save as walkthrough: after every change, the editor checks the game can still be won.`;
    } else if (w === "unchecked") {
      this.walk.textContent = "The walkthrough will be checked once the game's mistakes are fixed.";
    } else if (w.result === "win") {
      this.walk.className = "tp-walk ok";
      this.walk.textContent = `✓ The walkthrough still wins, in ${w.steps} command${w.steps === 1 ? "" : "s"}.`;
    } else {
      this.walk.className = "tp-walk bad";
      this.walk.textContent = `✗ ${w.problem ?? "The walkthrough doesn't win."}`.replace("✗ the", "✗ The");
    }
  }

  private saveWalkthrough(file = WALKTHROUGH) {
    if (!this.fromStart || !this.played.length) return;
    if (file === WALKTHROUGH && !this.won && !confirm("This game hasn't been won yet. Save what you've typed as the walkthrough anyway?")) return;
    const when = new Date().toISOString().slice(0, 10);
    const goal = file === WALKTHROUGH ? "" : ` It should reach the ${this.ending} ending.`;
    this.host.saveWalkthrough(
      `# Recorded in test play, ${when}.${goal} One command a line; lines starting with # are skipped.\n# A line can say what the game should answer: take key => Taken.\n${this.played.join("\n")}\n`,
      file,
    );
    this.say(`Saved your ${this.played.length} commands as ${file}.`, "notice");
  }

  /** Every walkthrough's result, and what none of them reach. */
  walkthroughs(list: Walkthrough[]) {
    const others = list.filter((w) => w.file && w.file !== WALKTHROUGH);
    this.walks.replaceChildren();
    for (const w of others) {
      const ok = !w.problem;
      this.walks.append(
        el("p", { className: `tp-walk ${ok ? "ok" : "bad"}`, textContent: ok ? `✓ ${w.file} still reaches the ${w.goal} ending, in ${w.steps} command${w.steps === 1 ? "" : "s"}.` : `✗ ${w.problem}` }),
      );
    }
    // What no walkthrough reaches: in all of their "never" lists.
    const nevers = list.map((w) => w.never).filter((n): n is NonNullable<Walkthrough["never"]> => !!n);
    if (!nevers.length) return;
    const all = <T>(get: (n: NonNullable<Walkthrough["never"]>) => T[], key: (x: T) => string) => get(nevers[0]).filter((x) => nevers.every((n) => get(n).some((y) => key(y) === key(x))));
    const rooms = all((n) => n.rooms, (x) => x);
    const things = all((n) => n.things, (x) => x);
    const options = all((n) => n.options, (o) => `${o.choice}.${o.node}.${o.option}`);
    if (!rooms.length && !things.length && !options.length) {
      this.walks.append(el("p", { className: "tp-walk ok", textContent: "✓ Between them, the walkthroughs go everywhere, see everything and try every reply." }));
      return;
    }
    const box = el("details", { className: "tp-never" });
    const total = rooms.length + things.length + options.length;
    box.append(el("summary", { textContent: `What the walkthrough${list.length === 1 ? "" : "s"} never reach${list.length === 1 ? "es" : ""} (${total})` }));
    box.append(el("p", { className: "dim", textContent: "Places never entered, things never seen, and replies never chosen. A change there isn't checked: worth a playthrough, or another walkthrough." }));
    const line = (label: string, items: HTMLElement[]) => {
      if (!items.length) return;
      const p = el("p", {}, el("strong", { textContent: `${label}: ` }));
      items.forEach((b, i) => p.append(...(i ? [", ", b] : [b])));
      box.append(p);
    };
    const link = (text: string, go: () => void) => {
      const b = el("button", { type: "button", className: "link", textContent: text });
      b.addEventListener("click", go);
      return b;
    };
    line("Rooms", rooms.map((r) => link(this.host.nameOf(r), () => this.host.show({ id: r }))));
    line("Things", things.map((t) => link(this.host.nameOf(t), () => this.host.show({ id: t }))));
    line("Replies", options.map((o) => link(`“${o.text || "…"}” (${o.choice.replace(/_/g, " ")})`, () => this.host.show({ choice: o.choice }))));
    this.walks.append(box);
  }

  /** The wanderer: random moves on games of their own, then what it found. */
  private async wander() {
    this.engine ??= await this.host.engine();
    const box = this.wanderBox;
    box.hidden = false;
    box.open = true;
    box.replaceChildren(el("summary", { textContent: "The wanderer" }), el("p", { className: "dim", textContent: "Wandering…" }));
    await new Promise((r) => setTimeout(r, 0));
    const w: Wandered = this.engine.wander(this.host.files(), ++this.wanderRuns, WANDER_MOVES);
    const out: (HTMLElement | string)[] = [el("summary", { textContent: w.error ? "The wanderer" : `The wanderer: ${w.errors.length ? `${w.errors.length} script error${w.errors.length === 1 ? "" : "s"}` : "no script errors"}, ${Object.keys(w.endings).length} ending${Object.keys(w.endings).length === 1 ? "" : "s"}` })];
    if (w.error) {
      box.replaceChildren(...out, el("p", { textContent: `It can't play yet: ${w.error}.` }));
      return;
    }
    out.push(el("p", { className: "dim", textContent: `${w.moves} random moves over ${w.games} game${w.games === 1 ? "" : "s"}, apart from yours. Wander again for different moves.` }));
    for (const e of w.errors) {
      const again = el("button", { type: "button", textContent: "Play it here" });
      again.title = "Play the moves that led to it in test play, to see it happen";
      again.addEventListener("click", () => void this.replay(e.commands, "moves"));
      out.push(el("p", { className: "tp-walk bad" }, `✗ A script error at move ${e.at} (${e.command}): ${e.text} `, again));
    }
    const ends = Object.entries(w.endings);
    out.push(el("p", {}, ends.length ? `Endings it reached: ${ends.map(([k, m]) => `${k} (first at move ${m})`).join(", ")}.` : "It didn't reach an ending."));
    out.push(el("p", {}, `It got to ${w.rooms[0]} of ${w.rooms[1]} rooms and saw ${w.things[0]} of ${w.things[1]} things${w.never.length ? `; never to ${w.never.map((r) => this.host.nameOf(r)).join(", ")}` : ""}.`));
    if (w.quiet && w.quiet.moves >= 200) {
      out.push(el("p", { className: "tp-walk" }, `Its longest stretch with nothing new: ${w.quiet.moves} moves from move ${w.quiet.from}, in ${this.host.nameOf(w.quiet.room)}. A player could get stuck there, or it's just hard to find the way on by chance.`));
    }
    box.replaceChildren(...out);
  }

  /** The game was edited: a game in progress may be out of date (moving rooms on the map doesn't count). */
  changed(now: Files) {
    if (!this.running || !this.startedWith) return;
    const game = (f: Files) => JSON.stringify(Object.entries(f).filter(([k]) => k !== "map.yaml" && !/^walkthrough(-[^/]+)?\.txt$/.test(k)).sort());
    this.stale.hidden = game(now) === game(this.startedWith);
  }

  async start(from?: string, quiet = false) {
    this.engine ??= await this.host.engine();
    let files = this.host.files();
    this.startedWith = files;
    if (from) {
      // The same game, starting somewhere else.
      try {
        const m = new Model(files["game.yaml"] ?? "");
        m.set(["game", "start"], from);
        files = { ...files, "game.yaml": m.toString() };
      } catch {
        /* the file doesn't parse: the checker will say so */
      }
    }
    this.log.replaceChildren();
    this.stale.hidden = true;
    this.played = [];
    this.fromStart = !from;
    this.won = false;
    this.ending = null;
    this.update();
    const loaded = this.engine.load(files, "game.yaml", SEED);
    if (!loaded.output) {
      const errors = loaded.diagnostics.filter((d) => d.severity === "error");
      const see = el("button", { type: "button", className: "link", textContent: "See the problems" });
      see.addEventListener("click", () => this.host.showProblems());
      this.log.append(el("p", {}, `This game can't be played until ${errors.length === 1 ? "its mistake is" : `its ${errors.length} mistakes are`} fixed. `, see));
      this.running = false;
      this.form.hidden = true;
      this.setHere(null);
      this.status.textContent = "";
      return;
    }
    this.running = true;
    this.form.hidden = false;
    if (from) this.say(`Started in ${this.host.selectedRoom()?.name ?? from}, as if the game began there.`, "notice");
    if (!quiet) this.show(loaded.output);
    this.line.focus({ preventScroll: true });
  }

  /**
   * Starts over and plays `commands` without showing them, then shows where
   * that got to: a step of the walkthrough, or the moves of the game in
   * progress on the new version. Stops early at an ending.
   */
  async replay(commands: string[], what: "walkthrough" | "moves") {
    await this.start(undefined, true);
    if (!this.running || !this.engine) return;
    let last: Output | null = null;
    let n = 0;
    for (const c of commands) {
      last = this.engine.input(c);
      this.played.push(c);
      n++;
      if (last.ended) break;
    }
    this.log.replaceChildren();
    const said = what === "walkthrough" ? `Played the walkthrough up to step ${n} (${commands[n - 1] ?? ""})` : `Played your ${n} move${n === 1 ? "" : "s"} again on the new version`;
    this.say(
      n < commands.length ? `${said}, but the game ended there, ${commands.length - n} short.` : `${said}. Carry on from here; Save as walkthrough keeps those moves too.`,
      "notice",
    );
    if (last) this.show(last);
    else this.say("(Nothing to play.)", "notice");
    this.update();
  }

  private send(text: string) {
    if (!this.engine || !this.running) return;
    this.say(text, "cmd");
    if (text) {
      this.played.push(text);
      this.update();
    }
    const room = this.here;
    const o = this.engine.input(text);
    this.show(o);
    if (o.missed && text) this.missed({ command: text, room, roomName: o.status.room, missed: o.missed });
  }

  /** Remembers a command that wasn't understood (once for each word and room). */
  private missed(m: Miss) {
    const same = (a: Miss) => a.missed.kind === m.missed.kind && (a.missed.word ?? a.command) === (m.missed.word ?? m.command) && a.room === m.room;
    if (this.misses.some(same)) return;
    this.misses.push(m);
    this.drawMisses();
  }

  private drawMisses() {
    const box = this.missBox;
    const wasOpen = box.open;
    box.hidden = !this.misses.length;
    box.replaceChildren(el("summary", { textContent: `Not understood (${this.misses.length})` }));
    box.append(el("p", { className: "dim", textContent: "Commands the game didn't understand, and a way to fix each. Players will try them too." }));
    const list = el("ul");
    for (const m of this.misses) list.append(this.missRow(m));
    box.append(list);
    box.open = wasOpen || this.misses.length === 1;
  }

  private missRow(m: Miss): HTMLElement {
    const row = el("li", {});
    const drop = () => {
      this.misses = this.misses.filter((x) => x !== m);
      this.drawMisses();
    };
    const done = (what: string) => {
      drop();
      this.say(`(${what}. Start over to play with it.)`, "notice");
    };
    const btn = (label: string, fn: () => void) => {
      const b = el("button", { type: "button", textContent: label });
      b.addEventListener("click", fn);
      return b;
    };
    const pick = (options: [string, string][], label: string) => {
      const s = el("select", {});
      s.setAttribute("aria-label", label);
      for (const [v, t] of options) s.append(el("option", { value: v, textContent: t }));
      return s;
    };
    const word = m.missed.word ?? "";
    const where = m.roomName ? ` (in ${m.roomName})` : "";
    row.append(el("code", { textContent: `> ${m.command}` }), where, " ");
    const fix = el("div", { className: "tp-fix" });
    switch (m.missed.kind) {
      case "unknown_word": {
        row.append(`“${word}” isn't a name for anything here.`);
        const things = m.room ? this.host.thingsIn(m.room) : [];
        if (things.length) {
          const s = pick(things.map((t) => [`${t.section}:${t.id}`, t.name]), `What “${word}” is`);
          fix.append(
            "It means ",
            s,
            btn("Add the word", () => {
              const [section, id] = s.value.split(":") as ["objects" | "characters", string];
              this.host.addSynonym(section, id, word);
              done(`“${word}” is another name for ${things.find((t) => `${t.section}:${t.id}` === s.value)?.name}`);
            }),
          );
        }
        if (m.room) fix.append(btn("Add it as something here", () => (this.host.addScenery(m.room!, word), done(`Added a ${word} to ${m.roomName}`))));
        break;
      }
      case "unknown_verb": {
        row.append(`“${word}” isn't a verb this game knows.`);
        const s = pick(this.host.verbWords().map((v) => [v, v.toUpperCase()]), `What “${word}” means`);
        fix.append(
          "It means ",
          s,
          btn("Make it mean that", () => (this.host.addAlias(word, s.value), done(`“${word}” now means ${s.value.toUpperCase()}`))),
          " or ",
          btn("Add a new verb", () => (this.host.addVerb(word), drop())),
        );
        break;
      }
      case "scenery":
        row.append(word ? `“${word}” is in the description, but isn't something the player can handle.` : "Those words are only in the room's description.");
        if (m.room && word) fix.append(btn("Make it an object", () => (this.host.addScenery(m.room!, word), done(`Added a ${word} to ${m.roomName}`))));
        break;
      case "not_here":
        row.append("That's in the game, but not here.");
        break;
      case "partial":
        row.append(`The game understood ${m.missed.verb ? `“${m.missed.verb.toUpperCase()}”` : "the start"}, but not the rest.`);
        break;
      default:
        row.append("The game didn't understand it.");
    }
    fix.append(btn("Dismiss", drop));
    row.append(fix);
    return row;
  }

  private say(text: string, cls = "") {
    this.log.append(el("p", { textContent: text, className: cls }));
  }

  private show(o: Output) {
    // Choices are numbered lines; the player types the number.
    for (const l of o.lines) {
      if (l.kind === "picture") this.log.append(figure(pictureSrc(l.picture ?? "", this.startedWith ?? {}), l.text));
      else this.say(l.text, l.kind === "text" ? "" : l.kind);
    }
    if (o.ended === "win") this.won = true;
    if (o.ended && o.ending) {
      this.ending = o.ending;
      this.update();
    }
    if (o.ended) this.say(`That's an ending${o.ending ? ` (${o.ending})` : ""}. UNDO goes back a move; Start over plays again.`, "notice");
    const s = o.status;
    this.status.textContent = [s.room, s.max_score !== null ? `score ${s.score}/${s.max_score}` : "", s.time ?? "", `turn ${s.turns}`].filter(Boolean).join(" · ");
    if (o.save || o.restore_requested) this.say("(Saving and restoring aren't kept in test play: use UNDO, or Start over.)", "notice");
    this.setHere(this.engine?.map().here ?? null);
    this.log.scrollTop = this.log.scrollHeight;
  }

  private setHere(room: string | null) {
    this.here = room;
    if (this.visible) this.host.markHere(room);
  }
}
