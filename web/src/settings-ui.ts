// The "Text and colours" dialog, shared by every page.
import { applySettings, settings, type Settings } from "./store.ts";

const CHOICES: { [K in keyof Settings]: [Settings[K], string][] } = {
  theme: [["auto", "Match my device"], ["light", "Light"], ["dark", "Dark"], ["sepia", "Sepia"]],
  size: [["small", "Small"], ["medium", "Medium"], ["large", "Large"]],
  face: [["serif", "Book"], ["sans", "Plain"]],
};
const LABELS: Record<keyof Settings, string> = { theme: "Colours", size: "Text size", face: "Typeface" };

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
      const row = document.createElement("div");
      row.className = "row";
      const label = document.createElement("label");
      label.textContent = LABELS[key];
      label.style.minWidth = "6rem";
      const select = document.createElement("select");
      select.id = label.htmlFor = `setting-${key}`;
      for (const [value, text] of CHOICES[key]) {
        const o = document.createElement("option");
        o.value = value;
        o.textContent = text;
        o.selected = s[key] === value;
        select.append(o);
      }
      select.addEventListener("change", () => {
        const next = { ...settings.get(), [key]: select.value };
        settings.set(next);
        applySettings(next);
      });
      row.append(label, select);
      dialog.append(row);
    }
  };
  opener.addEventListener("click", () => {
    render();
    dialog.showModal();
  });
}
