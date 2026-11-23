// Copies the example games from spec/examples into public/games/: one
// <id>.json of {path: text} per game, and index.json listing them.
import { mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { parse } from "yaml";

const root = fileURLToPath(new URL("../../", import.meta.url));
const examples = join(root, "spec/examples");
const out = fileURLToPath(new URL("../public/games/", import.meta.url));

// Shown first, in this order, with where each comes from.
const ORDER = {
  midnight_snack: "The README tutorial",
  lighthouse: "A sampler of every feature",
  campfire: "The scripting tutorial",
  blackfang: "The advanced tutorial",
};

function project(dir) {
  const files = {};
  const walk = (d) => {
    for (const name of readdirSync(d)) {
      const p = join(d, name);
      if (statSync(p).isDirectory()) walk(p);
      else if (/\.(ya?ml|lua)$/.test(name) || /^walkthrough(-[^/]+)?\.txt$/.test(name)) files[relative(dir, p).split("\\").join("/")] = readFileSync(p, "utf8");
      // Pictures, as data: URLs, as the site keeps them.
      else if (/\.(png|jpe?g|gif|webp|svg)$/i.test(name)) {
        const ext = name.split(".").pop().toLowerCase();
        const type = { png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", gif: "image/gif", webp: "image/webp", svg: "image/svg+xml" }[ext];
        files[relative(dir, p).split("\\").join("/")] = `data:${type};base64,${readFileSync(p).toString("base64")}`;
      }
    }
  };
  walk(dir);
  return files;
}

mkdirSync(out, { recursive: true });
const index = [];
for (const name of readdirSync(examples)) {
  const dir = join(examples, name);
  if (!statSync(join(dir, "game.yaml"), { throwIfNoEntry: false })) continue;
  const files = project(dir);
  const game = parse(files["game.yaml"]).game ?? {};
  const intro = typeof game.intro === "string" ? game.intro.replace(/\s+/g, " ").trim() : "";
  writeFileSync(join(out, `${name}.json`), JSON.stringify(files));
  index.push({ id: name, title: game.title ?? name, author: game.author ?? null, about: ORDER[name] ?? null, blurb: intro });
}
const rank = (id) => (id in ORDER ? Object.keys(ORDER).indexOf(id) : 99);
index.sort((a, b) => rank(a.id) - rank(b.id) || a.title.localeCompare(b.title));
writeFileSync(join(out, "index.json"), JSON.stringify(index, null, 2));
console.log(`bundled ${index.length} games into public/games/`);

// The editor's starter templates, from spec/templates, in one file:
// [{id, title, about, files}], in this order.
const STARTERS = {
  small_house: "Five rooms, a locked front door, and keys to find.",
  locked_dungeon: "A cell, a sleeping guard with the key, and an iron door to escape through.",
  market: "Someone to talk to, a baker to buy from, and someone to help.",
};
const starters = Object.entries(STARTERS).map(([id, about]) => {
  const files = project(join(root, "spec/templates", id));
  return { id, title: parse(files["game.yaml"]).game.title, about, files };
});
writeFileSync(join(out, "templates.json"), JSON.stringify(starters));
console.log(`bundled ${starters.length} starter templates into public/games/templates.json`);

// The standard verbs, for the editor's script picker: id and an example.
const stdlib = parse(readFileSync(join(root, "crates/eifgb-stdlib/stdlib/v1/en.yaml"), "utf8"));
const META = new Set(["inventory", "hint", "journal", "time", "score", "undo", "save", "restore", "restart", "help", "again", "quit", "health", "wait", "wait_for", "wait_until", "go_to", "go_back", "exits", "notes", "erase_note", "where_is", "oops", "verbose", "brief", "recap", "last_talk", "map", "inventory_by_place", "wares"]);
const verbs = Object.entries(stdlib.verbs ?? {})
  .filter(([id]) => !META.has(id))
  .map(([id, v]) => ({ id, example: (v.patterns?.[0] ?? id).replace(/\{(\w+)\}/g, (_, s) => (s === "target" ? "it" : s === "tool" ? "something" : "…")) }));
writeFileSync(join(out, "..", "verbs.json"), JSON.stringify(verbs));
console.log(`${verbs.length} verbs into public/verbs.json`);
