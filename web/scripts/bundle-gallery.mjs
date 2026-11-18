// Checks gallery.yaml and writes it to public/gallery.json for the Gallery
// page. Every mistake is reported, and any mistake fails the build.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { parse } from "yaml";

const web = fileURLToPath(new URL("../", import.meta.url));
const examples = fileURLToPath(new URL("../../spec/examples/", import.meta.url));
export const TAGS = ["short", "long", "beginner", "puzzle", "story", "combat", "mystery", "horror", "comedy", "fantasy", "sci-fi", "tutorial"];
const FIELDS = ["title", "author", "url", "example", "about", "tags"];

/** Checks the gallery; returns { games, errors }. */
export function check(text) {
  const errors = [];
  let doc;
  try {
    doc = parse(text);
  } catch (e) {
    return { games: [], errors: [`gallery.yaml isn't valid YAML: ${e.message}`] };
  }
  const list = doc?.games;
  if (!Array.isArray(list)) return { games: [], errors: ["gallery.yaml needs a `games:` list."] };
  const games = [];
  const seen = new Set();
  list.forEach((g, i) => {
    const at = `games[${i}]${typeof g?.title === "string" ? ` (${g.title})` : ""}`;
    const bad = (m) => errors.push(`${at}: ${m}`);
    if (!g || typeof g !== "object") return bad("each entry needs title, author, about and a url.");
    for (const k of Object.keys(g)) if (!FIELDS.includes(k)) bad(`unknown field \`${k}\`; the fields are ${FIELDS.join(", ")}.`);
    for (const k of ["title", "author", "about"]) if (typeof g[k] !== "string" || !g[k].trim()) bad(`\`${k}\` is missing.`);
    if (typeof g.title === "string" && g.title.length > 80) bad("`title` is over 80 characters.");
    if (typeof g.about === "string" && g.about.length > 300) bad("`about` is over 300 characters; one or two sentences is plenty.");
    if ((g.url === undefined) === (g.example === undefined)) bad(g.url === undefined ? "`url` is missing: where the game is hosted." : "give a `url` or an `example`, not both.");
    if (g.url !== undefined) {
      try {
        if (new URL(g.url).protocol !== "https:") bad("`url` must start with https://.");
      } catch {
        bad("`url` isn't a web address.");
      }
    }
    if (g.example !== undefined && !existsSync(join(examples, String(g.example), "game.yaml"))) bad(`there's no example game \`${g.example}\`.`);
    if (g.tags !== undefined) {
      if (!Array.isArray(g.tags)) bad("`tags` is a list, like [short, puzzle].");
      else for (const t of g.tags) if (!TAGS.includes(t)) bad(`unknown tag \`${t}\`; the tags are ${TAGS.join(", ")}.`);
    }
    const key = g.url ?? `example:${g.example}`;
    if (seen.has(key)) bad("this game is already in the gallery.");
    seen.add(key);
    games.push({ title: g.title, author: g.author, about: g.about, tags: g.tags ?? [], ...(g.url ? { url: g.url } : { example: g.example }) });
  });
  return { games, errors };
}

if (import.meta.url === `file://${process.argv[1]}` || process.argv[1]?.endsWith("bundle-gallery.mjs")) {
  const { games, errors } = check(readFileSync(join(web, "gallery.yaml"), "utf8"));
  for (const e of errors) console.error(`error: gallery.yaml: ${e}`);
  if (errors.length) process.exit(1);
  mkdirSync(join(web, "public"), { recursive: true });
  writeFileSync(join(web, "public/gallery.json"), JSON.stringify({ tags: TAGS, games }, null, 2));
  console.log(`gallery: ${games.length} games`);
}
