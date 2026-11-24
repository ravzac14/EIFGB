// A game as one web page: the engine, its WebAssembly, a small player and
// the game's files, all inside a single .html file. It plays offline,
// opened straight from a disk, and can be hosted or emailed anywhere.
import type { Files } from "./engine.ts";
import playerJs from "./standalone/player.js?raw";
import tabJs from "./tab.js?raw";
import siteCss from "./style.css?raw";
import playCss from "./play.css?raw";

/** What the page needs from the site: the engine's two files. */
export interface EngineFiles {
  /** The engine's loader, eifgb.mjs, as text. */
  loader: string;
  /** The engine's WebAssembly, eifgb_web.wasm. */
  wasm: Uint8Array;
}

/** Fetches the engine's files from the site's `wasm/` folder. */
export async function fetchEngineFiles(base: string | URL): Promise<EngineFiles> {
  const [loader, wasm] = await Promise.all([
    fetch(new URL("eifgb.mjs", base)).then((r) => r.text()),
    fetch(new URL("eifgb_web.wasm", base)).then((r) => r.arrayBuffer()),
  ]);
  return { loader, wasm: new Uint8Array(wasm) };
}

function base64(bytes: Uint8Array): string {
  let s = "";
  // In pieces: one call with every byte would overflow the stack.
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
}

const html = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
/** Text that's safe inside a <script> element. */
const inScript = (s: string) => s.replace(/<\/(script)/gi, "<\\/$1").replace(/<!--/g, "<\\!--");

/**
 * The engine's loader is an ES module; the page runs it as an ordinary
 * script (modules can't be loaded from a file on disk), with the
 * WebAssembly handed over directly instead of fetched.
 */
export function classicLoader(loader: string): string {
  const out = loader.replaceAll("import.meta.url", "document.baseURI").replace(/export\s+default\s+(\w+)\s*;?/, "window.createEifgb = $1;");
  if (!out.includes("window.createEifgb")) throw new Error("the engine's loader isn't in the expected form");
  return out;
}

/**
 * The page. `title` is the game's; `site` is the address of the site
 * that made it, for the "Made with" line (left out when not given).
 */
export function standalonePage(files: Files, title: string, engine: EngineFiles, site?: string): string {
  const game = JSON.stringify(files).replace(/</g, "\\u003c");
  const made = site ? `<p class="made dim">Made with <a href="${html(site)}">EIFGB</a>.</p>` : "";
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="generator" content="EIFGB">
<title>${html(title)}</title>
<style>
${siteCss}
${playCss}
.standalone .bar { justify-content: space-between; }
.standalone .made { margin: 3rem 0 1rem; font-size: 0.85em; text-align: center; }
</style>
</head>
<body class="standalone">
<header class="bar ui">
  <span id="title" class="title">${html(title)}</span>
  <span id="status" class="status" aria-live="off"></span>
</header>
<main class="wrap">
  <div id="log" class="log" aria-live="polite"><p class="notice">Loading the game…</p></div>
  <form id="prompt" class="prompt" hidden>
    <label for="line" class="caret">&gt;</label>
    <input id="line" autocomplete="off" autocapitalize="off" autocorrect="off" spellcheck="false" enterkeyhint="send" aria-label="What do you do?">
  </form>
  ${made}
</main>
<script type="application/json" id="eifgb-game">${game}</script>
<script type="application/octet-stream" id="eifgb-wasm">${base64(engine.wasm)}</script>
<script>${inScript(classicLoader(engine.loader))}</script>
<script>${inScript(tabJs)}</script>
<script>${inScript(playerJs)}</script>
</body>
</html>
`;
}
