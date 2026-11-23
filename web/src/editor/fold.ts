// Folding the details panel: a panel opens with what most things need
// (name, description, where it is), and each further section folded, its
// title saying what's set in it ("What it does · worn, gives light"), so
// nothing is hidden that matters and nothing is in the way that doesn't.
// Lists of what's here (ways out, things in a room) stay open. Which
// sections an author opened is remembered while they work, so a redraw
// after every change doesn't fold them again.

/** Sections that are always open: they're the lists of what's there. */
const ALWAYS_OPEN = new Set(["Ways out", "In this room", "People and creatures", "Carried at the start", "Not placed anywhere yet", "For sale", "Carrying", "Members", "Getting started"]);

/** Which sections were opened (or closed), by panel and title. */
const remembered = new Map<string, boolean>();

/** `sel` in `b`, and `b` itself if it is one. */
const all = <T extends Element>(b: Element, sel: string) => [...(b.matches(sel) ? [b as T] : []), ...b.querySelectorAll<T>(sel)];

/** What's set in a section, in a few words: the boxes ticked, the choice made. */
function whatsSet(body: Element[]): string {
  const said: string[] = [];
  for (const b of body) {
    for (const input of all<HTMLInputElement>(b, "input[type=checkbox], input[type=radio]")) {
      if (!input.checked) continue;
      const label = input.closest("label")?.textContent?.trim();
      if (label && !said.includes(label)) said.push(label);
    }
  }
  // A choice made from a list ("Friendly: cheaper" says "Friendly").
  for (const b of body) {
    for (const sel of all<HTMLSelectElement>(b, "select")) {
      const first = sel.options[0];
      const chosen = sel.selectedOptions[0];
      if (!chosen || chosen === first || !first || first.value !== "") continue;
      const text = chosen.textContent?.split(":")[0].trim();
      if (text && !said.includes(text)) said.push(text);
    }
  }
  // A list: how many are in it.
  const items = body.flatMap((b) => all(b, "ul.things")).reduce((n, ul) => n + [...ul.children].filter((c) => c.tagName === "LI").length, 0);
  if (!said.length && items) said.push(`${items}`);
  if (!said.length) return "";
  const short = said.map((s) => (s.length > 32 ? `${s.slice(0, 30)}…` : s));
  return short.length > 3 ? `${short.slice(0, 3).join(", ")} and ${short.length - 3} more` : short.join(", ");
}

function folder(title: string, summaryNote: string, key: string, body: Element[]): HTMLDetailsElement {
  const d = document.createElement("details");
  d.className = "section";
  const s = document.createElement("summary");
  s.append(title);
  if (summaryNote) {
    const note = document.createElement("span");
    note.className = "sum";
    note.textContent = ` · ${summaryNote}`;
    s.append(note);
  }
  d.append(s, ...body);
  d.open = remembered.get(key) ?? false;
  d.addEventListener("toggle", () => remembered.set(key, d.open));
  return d;
}

/**
 * Folds a drawn panel. `kind` names the panel ("room", "thing"…), so each
 * kind remembers its own open sections.
 */
export function foldSections(panel: HTMLElement, kind: string) {
  const kids = [...panel.children];
  for (let i = 0; i < kids.length; i++) {
    const h = kids[i];
    // The game's story sections: folded unless opened, with how many each has.
    if (h.matches("details.group")) {
      const d = h as HTMLDetailsElement;
      const summary = [...d.children].find((c) => c.tagName === "SUMMARY") ?? null;
      const title = summary?.textContent?.trim() ?? "";
      const key = `${kind}:${title}`;
      const n = whatsSet([...d.children].filter((c) => c !== summary));
      if (summary && n) {
        const note = document.createElement("span");
        note.className = "sum";
        note.textContent = ` · ${n}`;
        summary.append(note);
      }
      d.open = remembered.get(key) ?? false;
      d.addEventListener("toggle", () => remembered.set(key, d.open));
      continue;
    }
    if (h.tagName !== "H3") continue;
    const title = h.textContent?.trim() ?? "";
    if (ALWAYS_OPEN.has(title)) continue;
    // The section: everything up to the next heading, story group, or "More".
    const body: Element[] = [];
    let j = i + 1;
    for (; j < kids.length; j++) {
      const k = kids[j];
      if (k.tagName === "H3" || k.matches("details.group") || (k.tagName === "DETAILS" && /^More/.test(k.querySelector("summary")?.textContent ?? ""))) break;
      body.push(k);
    }
    const d = folder(title, whatsSet(body), `${kind}:${title}`, body);
    h.replaceWith(d);
    i = j - 1;
  }
}
