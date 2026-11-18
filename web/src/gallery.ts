// The Gallery page: games from gallery.yaml, and making a share link.
import { settingsDialog } from "./settings-ui.ts";
import { savesFor } from "./store.ts";
import { fetchGame, resolve } from "./remote.ts";
import { loadEngine } from "./engine.ts";

interface Game {
  title: string;
  author: string;
  about: string;
  tags: string[];
  url?: string;
  example?: string;
}

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
settingsDialog($<HTMLDialogElement>("settings"), $("settings-open"));

function el<K extends keyof HTMLElementTagNameMap>(tag: K, props: Partial<HTMLElementTagNameMap[K]> = {}, ...kids: (Node | string)[]) {
  const e = Object.assign(document.createElement(tag), props);
  e.append(...kids);
  return e;
}

/** The play page's address for a game link, from wherever the site is hosted. */
function playLink(url: string) {
  return new URL(`play.html?game=${encodeURIComponent(url)}`, location.href).href;
}

function href(g: Game) {
  return g.example ? `./play.html?example=${encodeURIComponent(g.example)}` : `./play.html?game=${encodeURIComponent(g.url!)}`;
}

function saveKey(g: Game) {
  if (g.example) return `example:${g.example}`;
  try {
    const w = resolve(g.url!);
    return `url:${w.kind === "gist" ? `gist:${w.id}` : w.url}`;
  } catch {
    return "";
  }
}

let games: Game[] = [];
const picked = new Set<string>();

function show() {
  const q = $<HTMLInputElement>("search").value.trim().toLowerCase();
  const shown = games.filter(
    (g) =>
      [...picked].every((t) => g.tags.includes(t)) &&
      (!q || [g.title, g.author, g.about, ...g.tags].some((s) => s.toLowerCase().includes(q))),
  );
  const list = $("games");
  if (!shown.length) {
    list.replaceChildren(el("li", { className: "dim", textContent: "No games match." }));
    return;
  }
  list.replaceChildren(
    ...shown.map((g) => {
      const li = el("li");
      li.append(el("a", { className: "title", href: href(g), textContent: g.title }), " ", el("span", { className: "by", textContent: `by ${g.author}` }));
      li.append(el("p", { textContent: g.about }));
      const actions = el("div", { className: "actions" });
      actions.append(el("a", { href: href(g), textContent: savesFor(saveKey(g)).auto() ? "Continue" : "Play" }));
      for (const t of g.tags) actions.append(el("span", { className: "tag", textContent: t }));
      li.append(actions);
      return li;
    }),
  );
}

async function loadGallery() {
  try {
    const data: { tags: string[]; games: Game[] } = await (await fetch("./gallery.json")).json();
    games = data.games;
    const used = data.tags.filter((t) => games.some((g) => g.tags.includes(t)));
    $("tags").replaceChildren(
      ...used.map((t) => {
        const b = el("button", { type: "button", textContent: t });
        b.setAttribute("aria-pressed", "false");
        b.addEventListener("click", () => {
          if (picked.has(t)) picked.delete(t);
          else picked.add(t);
          b.setAttribute("aria-pressed", String(picked.has(t)));
          show();
        });
        return b;
      }),
    );
    show();
  } catch {
    $("games").replaceChildren(el("li", { className: "dim", textContent: "The gallery couldn't be loaded." }));
  }
}

$("search").addEventListener("input", show);

// ------------------------------------------------------- sharing a game

let engine: ReturnType<typeof loadEngine> | null = null;

$("share").addEventListener("submit", async (e) => {
  e.preventDefault();
  const out = $("share-result");
  const url = $<HTMLInputElement>("share-url").value.trim();
  out.replaceChildren(el("p", { className: "dim", textContent: "Fetching and checking the game…" }));
  try {
    const files = await fetchGame(url);
    engine ??= loadEngine(new URL("./wasm/", location.href));
    const { diagnostics } = (await engine).check(files);
    const errors = diagnostics.filter((d) => d.severity === "error");
    if (errors.length) {
      const ul = el("ul");
      for (const d of errors) ul.append(el("li", {}, ...(d.path ? [el("code", { textContent: d.path }), " "] : []), d.message));
      out.replaceChildren(el("p", { className: "bad", textContent: "The game was found, but it has mistakes to fix first:" }), ul);
      return;
    }
    const link = playLink(url);
    const box = document.createElement("input");
    box.readOnly = true;
    box.value = link;
    box.setAttribute("aria-label", "Play link");
    box.addEventListener("focus", () => box.select());
    const copy = el("button", { type: "button", textContent: "Copy link" });
    copy.addEventListener("click", async () => {
      try {
        await navigator.clipboard.writeText(link);
        copy.textContent = "Copied";
      } catch {
        box.select();
      }
    });
    const n = Object.keys(files).length;
    const warn = diagnostics.length ? ` It has ${diagnostics.length} warning${diagnostics.length > 1 ? "s" : ""}, which players won't mind.` : "";
    out.replaceChildren(
      el("p", { className: "ok", textContent: `It works: ${n} file${n > 1 ? "s" : ""}, no mistakes.${warn} Anyone with this link can play it:` }),
      box,
      el("div", { className: "actions" }, copy, " ", el("a", { href: link, textContent: "Play it now" })),
    );
  } catch (err) {
    out.replaceChildren(el("p", { className: "bad", textContent: err instanceof Error ? err.message : String(err) }));
  }
});

loadGallery();
