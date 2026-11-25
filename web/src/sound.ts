// A game's sounds in the web player: a room's own sound (`sound_file:` on a
// room), played over and over while the player is there, and sounds played
// once (a timer's `sound_file:`, a script's `play_sound`). Off unless the
// player turns Sound on in "Text and colours"; the story's words always say
// what happens too, so nothing is missed with it off.
import { soundSrc } from "./pictures.ts";
import { settings } from "./store.ts";
import type { Files } from "./engine.ts";

const VOLUME = { off: 0, quiet: 0.3, on: 0.8 } as const;
/** How long a room's sound takes to fade in or out, in milliseconds. */
const FADE = 600;

export interface SoundPlayer {
  /** The room's sound now (or none): carries on if it's the same one. */
  ambient(file: string | null | undefined): void;
  /** Sounds to play once. */
  play(files: string[] | undefined): void;
  /** Stops everything (a new story, leaving the page). */
  stop(): void;
  /** Whether the game has played, or tried to play, any sound yet. */
  heard(): boolean;
}

export function soundPlayer(files: () => Files): SoundPlayer {
  let room: { file: string; audio: HTMLAudioElement | null } | null = null;
  let any = false;
  const volume = () => VOLUME[settings.get().sound] ?? 0;

  function fade(a: HTMLAudioElement, to: number, then?: () => void) {
    const from = a.volume;
    const start = performance.now();
    const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
    const step = (now: number) => {
      const t = reduce ? 1 : Math.min(1, (now - start) / FADE);
      a.volume = Math.max(0, Math.min(1, from + (to - from) * t));
      if (t < 1) requestAnimationFrame(step);
      else then?.();
    };
    requestAnimationFrame(step);
  }

  function startRoom() {
    if (!room || room.audio || !volume()) return;
    const src = soundSrc(room.file, files());
    if (!src) return;
    const a = new Audio(src);
    a.loop = true;
    a.volume = 0;
    room.audio = a;
    a.play().then(
      () => fade(a, volume()),
      () => {
        // The browser wouldn't play it yet (no click or key so far): try again next time.
        if (room?.audio === a) room.audio = null;
      },
    );
  }

  function stopRoom() {
    const a = room?.audio;
    if (room) room.audio = null;
    if (a) fade(a, 0, () => a.pause());
  }

  // The player changed the setting: louder, quieter, on or off now.
  addEventListener("eifgb-sound", () => {
    const v = volume();
    if (!v) return stopRoom();
    if (room?.audio) room.audio.volume = v;
    else startRoom();
  });

  return {
    ambient(file) {
      if (file) any = true;
      if ((room?.file ?? null) === (file ?? null)) return startRoom();
      stopRoom();
      room = file ? { file, audio: null } : null;
      startRoom();
    },
    play(list) {
      if (!list?.length) return;
      any = true;
      const v = volume();
      if (!v) return;
      for (const f of list) {
        const src = soundSrc(f, files());
        if (!src) continue;
        const a = new Audio(src);
        a.volume = v;
        a.play().catch(() => {});
      }
    },
    stop() {
      stopRoom();
      room = null;
    },
    heard: () => any,
  };
}
