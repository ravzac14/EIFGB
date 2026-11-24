// The "Text and colours" dialog, shared by every page.
import { applySettings, getGameLook, settings, type Settings } from "./store.ts";
import { canSpeak, stopSpeaking } from "./speech.ts";
// A typeface made for legibility, served with the site (not from a font service).
import "@fontsource/atkinson-hyperlegible/400.css";
import "@fontsource/atkinson-hyperlegible/700.css";
// Made for readers with dyslexia; loaded only if chosen.
import "@fontsource/opendyslexic/400.css";
import "@fontsource/opendyslexic/700.css";

const CHOICES: { [K in keyof Settings]: [Settings[K], string][] } = {
  theme: [
    ["auto", "Match my device"],
    ["light", "Light"],
    ["dark", "Dark"],
    ["sepia", "Sepia"],
    ["parchment", "Parchment"],
    ["terminal", "Terminal (green)"],
    ["amber", "Terminal (amber)"],
    ["contrast", "High contrast"],
  ],
  size: [["small", "Small"], ["medium", "Medium"], ["large", "Large"], ["xlarge", "Very large"]],
  face: [["serif", "Book"], ["sans", "Plain"], ["legible", "Very legible"], ["dyslexic", "OpenDyslexic (for dyslexia)"]],
  spacing: [["normal", "Normal"], ["relaxed", "Roomy"], ["roomiest", "Roomiest, letters and words too"]],
  width: [["narrow", "Narrow"], ["medium", "Medium"], ["wide", "Wide"]],
  reveal: [["instant", "All at once"], ["typewriter", "Letter by letter"], ["sentence", "A sentence at a time (Enter for the next)"]],
  speech: [["off", "Off"], ["on", "Read new text aloud"], ["slow", "Read aloud, slower"], ["fast", "Read aloud, faster"]],
};
const LABELS: Record<keyof Settings, string> = { theme: "Colours", size: "Text size", face: "Typeface", spacing: "Line spacing", width: "Column width", reveal: "Text appears", speech: "Read aloud" };
/** Settings that only mean something while playing: shown on the play page. */
const PLAYER_ONLY = new Set<keyof Settings>(["reveal", "speech"]);

export function settingsDialog(dialog: HTMLDialogElement, opener: HTMLElement) {
  applySettings();
  const render = () => {
    const s = settings.get();
    dialog.replaceChildren();
    const close = Object.assign(document.createElement("button"), { className: "close", type: "button", textContent: "Done" });
    close.addEventListener("click", () => dialog.close());
    const h = Object.assign(document.createElement("h2"), { textContent: "Text and colours" });
    dialog.append(close, h);
    for (const key of Object.keys(CHOICES) as (keyof Settings)[]) {
      if (PLAYER_ONLY.has(key) && !document.getElementById("log")) continue;
      if (key === "speech" && !canSpeak()) continue;
      const row = document.createElement("div");
      row.className = "row";
      const label = document.createElement("label");
      label.textContent = LABELS[key];
      label.style.minWidth = "6rem";
      const select = document.createElement("select");
      select.id = label.htmlFor = `setting-${key}`;
      const look = getGameLook();
      for (const [value, text] of CHOICES[key]) {
        const o = document.createElement("option");
        o.value = value;
        o.textContent = text;
        // Playing a game with its own look: "auto" means the game's choice.
        if (key === "theme" && value === "auto" && look) {
          const named = CHOICES.theme.find(([v]) => v === look.theme)?.[1];
          o.textContent = named ? `The game's own (${named})` : "The game's own";
        }
        o.selected = s[key] === value;
        select.append(o);
      }
      select.addEventListener("change", () => {
        const next = { ...settings.get(), [key]: select.value };
        settings.set(next);
        applySettings(next);
        if (key === "speech") stopSpeaking();
      });
      row.append(label, select);
      dialog.append(row);
      if (key === "speech") {
        const note = Object.assign(document.createElement("p"), {
          className: "help dim",
          textContent: "Uses your browser's own voices. Escape stops reading. Screen readers already read new text, so you won't need this with one.",
        });
        dialog.append(note);
      }
    }
  };
  opener.addEventListener("click", () => {
    render();
    dialog.showModal();
  });
}
