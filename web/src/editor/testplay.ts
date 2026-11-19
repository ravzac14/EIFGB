// Test play: the game, played in the editor's drawer, so trying something
// doesn't mean switching tabs. It plays the game as it is when started (or
// started over), from the start room or from the room selected on the map,
// and shows on the map which room the player is in.

import type { Engine, Files, Output } from "../engine.ts";
import { Model } from "./model.ts";

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
    panel.append(el("div", { className: "tp-bar" }, restart, this.fromRoom, this.status), this.stale, this.log, this.form);
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
  }

  /** The game was edited: a game in progress may be out of date (moving rooms on the map doesn't count). */
  changed(now: Files) {
    if (!this.running || !this.startedWith) return;
    const game = (f: Files) => JSON.stringify(Object.entries(f).filter(([k]) => k !== "map.yaml").sort());
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
    const loaded = this.engine.load(files);
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
    this.show(this.engine.input(text));
  }

  private say(text: string, cls = "") {
    this.log.append(el("p", { textContent: text, className: cls }));
  }

  private show(o: Output) {
    // Choices are numbered lines; the player types the number.
    for (const l of o.lines) this.say(l.text, l.kind === "text" ? "" : l.kind);
    if (o.ended) this.say(`That's an ending${o.ending ? ` (${o.ending})` : ""}. UNDO goes back a move; Start over plays again.`, "notice");
    const s = o.status;
    this.status.textContent = [s.room, s.max_score !== null ? `score ${s.score}/${s.max_score}` : "", `turn ${s.turns}`].filter(Boolean).join(" · ");
    if (o.save || o.restore_requested) this.say("(Saving and restoring aren't kept in test play: use UNDO, or Start over.)", "notice");
    this.setHere(this.engine?.map().here ?? null);
    this.log.scrollTop = this.log.scrollHeight;
  }

  private setHere(room: string | null) {
    this.here = room;
    if (this.visible) this.host.markHere(room);
  }
}
