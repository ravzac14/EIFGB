// "Read aloud", in Text and colours on the play page: new text is spoken
// with the browser's own voices. Nothing is sent anywhere; the browser (or
// the device) does the speaking. Screen reader users don't need it: the
// story's log is a live region, so screen readers read new text already.

import type { Speech } from "./store.ts";

const RATE: Record<Exclude<Speech, "off">, number> = { on: 1, slow: 0.8, fast: 1.3 };

/** Whether this browser can speak. */
export function canSpeak(): boolean {
  return typeof window !== "undefined" && "speechSynthesis" in window && typeof SpeechSynthesisUtterance !== "undefined";
}

/**
 * Speaks lines of text, after stopping anything still being read: what's
 * new matters more than finishing what came before.
 */
export function speak(lines: string[], how: Speech, lang = "en") {
  if (how === "off" || !canSpeak()) return;
  speechSynthesis.cancel();
  for (const line of lines) {
    const text = line.trim();
    if (!text) continue;
    const u = new SpeechSynthesisUtterance(text);
    u.lang = lang;
    u.rate = RATE[how];
    speechSynthesis.speak(u);
  }
}

/** Stops reading. */
export function stopSpeaking() {
  if (canSpeak()) speechSynthesis.cancel();
}
