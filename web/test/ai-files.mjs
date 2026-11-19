// The files for AI assistants and crawlers: llms.txt, llms-full.txt,
// sitemap.xml and the Markdown copy of every docs page. Every address in
// them must exist in the built site. Run after prepare-site:
// node test/ai-files.mjs
import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";

const pub = fileURLToPath(new URL("../public/", import.meta.url));
const web = fileURLToPath(new URL("../", import.meta.url));
const SITE = "https://ravzac14.github.io/EIFGB/";
/** Where an address on the site is in the build: public/ files, or the site's own pages. */
const exists = (url) => {
  const path = url.slice(SITE.length).split(/[?#]/)[0] || "index.html";
  return existsSync(pub + path) || existsSync(web + path);
};

const llms = readFileSync(pub + "llms.txt", "utf8");
assert.match(llms, /^# .+\n\n> .+/, "a title, then a one-line summary (llmstxt.org)");
const linked = [...llms.matchAll(/\]\((https:[^)]+)\)/g)].map((m) => m[1]).filter((u) => u.startsWith(SITE));
assert.ok(linked.length >= 10, "links to the docs and examples");
for (const u of linked) assert.ok(exists(u), `llms.txt links to ${u}, which isn't built`);
assert.match(llms, /docs\/ai\.md/);

const full = readFileSync(pub + "llms-full.txt", "utf8");
assert.ok(full.startsWith(`<!-- ${SITE}docs/ai.md -->\n\n# Writing games with an AI assistant`), "the guide for AI assistants comes first");
assert.match(full, /# Format reference|# EIFGB Source Format/);

const sitemap = readFileSync(pub + "sitemap.xml", "utf8");
const locs = [...sitemap.matchAll(/<loc>([^<]+)<\/loc>/g)].map((m) => m[1]);
assert.ok(locs.length >= 10);
for (const u of locs) assert.ok(exists(u), `sitemap.xml lists ${u}, which isn't built`);

// Every docs page says where its Markdown is, and it's there.
for (const page of readdirSync(pub + "docs").filter((f) => f.endsWith(".html"))) {
  const md = readFileSync(pub + "docs/" + page, "utf8").match(/<link rel="alternate" type="text\/markdown" href="([^"]+)"/)?.[1];
  assert.ok(md && existsSync(pub + "docs/" + md), `${page} has a Markdown copy`);
}
console.log("ok   files for AI assistants");
