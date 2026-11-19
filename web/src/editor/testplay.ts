// Test play: the game, played in the editor's drawer, so trying something
// doesn't mean switching tabs. It plays the game as it is when started (or
// started over), from the start room or from the room selected on the map,
// and shows on the map which room the player is in.
//
// What's typed in a game played from the start can be saved as the
// game's walkthrough.txt; the editor's check replays it after every edit
// and says here (and under Problems) when it no longer wins.

import type { Engine, Files, Output, Walkthrough } from "../engine.ts";
import { Model } from "./model.ts";

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
  /** Writes walkthrough.txt. */
  saveWalkthrough(text: string): void;
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
  private walk: HTMLElement;

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
    this.walk = el("p", { className: "tp-walk dim" });
    this.status = el("span", { className: "dim tp-status", role: "status" });
    this.stale = el("p", { className: "tp-stale", hidden: true });
    const again = el("button", { type: "button", className: "link", textContent: "Start over" });
    again.addEventListener("click", () => void this.start());
    this.stale.append("You've changed the game since this started. ", again, " to play the new version.");
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
    panel.append(el("div", { className: "tp-bar" }, restart, this.fromRoom, this.record, this.status), this.walk, this.stale, this.log, this.form);
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
    this.record.title = !this.fromStart
      ? "Only a game played from the start can be a walkthrough"
      : `Save the ${this.played.length} commands you've typed as ${WALKTHROUGH}, to check after every change that the game can still be won`;
  }

  /** How the saved walkthrough plays now: "none" when there isn't one, "unchecked" when the game can't be played. */
  walkthrough(w: Walkthrough | "none" | "unchecked") {
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

  private saveWalkthrough() {
    if (!this.fromStart || !this.played.length) return;
    if (!this.won && !confirm("This game hasn't been won yet. Save what you've typed as the walkthrough anyway?")) return;
    const when = new Date().toISOString().slice(0, 10);
    this.host.saveWalkthrough(`# Recorded in test play, ${when}. One command a line; lines starting with # are skipped.\n${this.played.join("\n")}\n`);
    this.say(`Saved your ${this.played.length} commands as ${WALKTHROUGH}.`, "notice");
  }

  /** The game was edited: a game in progress may be out of date (moving rooms on the map doesn't count). */
  changed(now: Files) {
    if (!this.running || !this.startedWith) return;
    const game = (f: Files) => JSON.stringify(Object.entries(f).filter(([k]) => k !== "map.yaml" && k !== WALKTHROUGH).sort());
    this.stale.hidden = game(now) === game(this.startedWith);
  }

  async start(from?: string) {
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
    this.show(loaded.output);
    this.line.focus({ preventScroll: true });
  }

  private send(text: string) {
    if (!this.engine || !this.running) return;
    this.say(text, "cmd");
    if (text) {
      this.played.push(text);
      this.update();
    }
    this.show(this.engine.input(text));
  }

  private say(text: string, cls = "") {
    this.log.append(el("p", { textContent: text, className: cls }));
  }

  private show(o: Output) {
    // Choices are numbered lines; the player types the number.
    for (const l of o.lines) this.say(l.text, l.kind === "text" ? "" : l.kind);
    if (o.ended === "win") this.won = true;
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
