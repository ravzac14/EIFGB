// Making a game with an AI assistant in a chat: the instructions to give
// it, its reply turned into a game (whatever's around the code), and the
// checker's problems turned into a message to send back.

import type { Diagnostic, Walkthrough } from "../engine.ts";

/** Where the guide for AI assistants is, on the site. */
export const AI_GUIDE = "https://ravzac14.github.io/EIFGB/docs/ai.md";

/** The fenced blocks in a chat reply: ```yaml … ``` and so on, with their language. */
function blocks(text: string): { lang: string; body: string }[] {
  const out: { lang: string; body: string }[] = [];
  for (const m of text.matchAll(/^(`{3,}|~{3,})[ \t]*([\w-]*)[^\n]*\n([\s\S]*?)^\1[ \t]*$/gm)) out.push({ lang: m[2].toLowerCase(), body: m[3] });
  return out;
}

/**
 * A game from a chat reply: the YAML block that's a game (it has
 * `format:`), and the walkthrough if there's a text block after it. Text
 * that's already just a game is taken as it is. Null if there's no game in it.
 */
export function fromChat(text: string): { yaml: string; walkthrough?: string } | null {
  const trimmed = text.trim();
  if (/^format:/m.test(trimmed) && !/^(`{3,}|~{3,})/m.test(trimmed)) return { yaml: `${trimmed}\n` };
  const bs = blocks(text);
  const game = bs.find((b) => /^format:\s*eifgb/m.test(b.body)) ?? bs.find((b) => /^(yaml|yml)$/.test(b.lang) && /^(game|rooms):/m.test(b.body));
  if (!game) return null;
  const rest = bs.slice(bs.indexOf(game) + 1);
  const walk = rest.find((b) => (b.lang === "text" || b.lang === "txt" || b.lang === "") && !/^format:/m.test(b.body) && b.body.trim());
  return { yaml: game.body.replace(/\s*$/, "\n"), ...(walk ? { walkthrough: walk.body.replace(/\s*$/, "\n") } : {}) };
}

/** What to paste into a chat first: what to do, then the guide itself (so it needn't fetch anything). */
export function instructions(guide: string | null): string {
  return [
    "I'd like your help writing a text adventure for the Interactive Fiction Game Builder (EIFGB), which plays it from a single YAML file.",
    "",
    "My idea: (describe your story or world here: where it happens, who's in it, what the player is trying to do, how it can end)",
    "",
    "Please:",
    "- Reply with the whole game as one ```yaml block, from `format:` to the end, every time (not a part or a change).",
    "- Keep it to that one file: write any Lua inline in handlers.",
    "- Add a ```text block after it: a walkthrough, the commands that win it, one a line.",
    "- When I send you problems the checker found, fix every one and send the whole file again.",
    "",
    guide ? `Here is the guide to the format:\n\n${guide.trim()}` : `The guide to the format is at ${AI_GUIDE}: read it before you start.`,
    "",
  ].join("\n");
}

/** The checker's problems, and how the walkthrough went, as a message to send back. */
export function forChat(problems: Diagnostic[], walkthroughs: Walkthrough[] = []): string {
  const errors = problems.filter((p) => p.severity === "error");
  const warnings = problems.filter((p) => p.severity !== "error");
  const lines: string[] = [];
  if (!problems.length) {
    lines.push("The checker found no problems.");
  } else {
    lines.push(`The EIFGB checker found ${errors.length ? `${errors.length} error${errors.length === 1 ? "" : "s"}` : ""}${errors.length && warnings.length ? " and " : ""}${warnings.length ? `${warnings.length} warning${warnings.length === 1 ? "" : "s"}` : ""} in the game:`, "");
    for (const p of [...errors, ...warnings]) lines.push(`- ${p.severity === "error" ? "Error" : "Warning"}${p.path ? ` at ${p.path}` : ""}: ${p.message}`);
  }
  const played = walkthroughs.filter((w) => w.file);
  if (played.length) {
    lines.push("");
    for (const w of played) lines.push(w.problem ? `- ${w.file}: ${w.problem}` : `- ${w.file} plays through, in ${w.steps} commands.`);
  }
  if (problems.length) lines.push("", "Please fix them all, and send the whole game.yaml again, in one ```yaml block.");
  return lines.join("\n");
}

/** Copies text, with or without the clipboard API. */
export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.append(ta);
    ta.select();
    let ok = false;
    try {
      ok = document.execCommand("copy");
    } catch {
      ok = false;
    }
    ta.remove();
    return ok;
  }
}
