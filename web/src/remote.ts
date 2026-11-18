// Games played from a link: `play.html?game=<url>`. The URL can be a raw
// game.yaml, a folder holding one, a .zip, a GitHub file/folder/repository
// page, or a gist. A game.yaml's `scripts:` are fetched from beside it.

import type { Files } from "./engine.ts";
import { fromZipBytes } from "./files.ts";

/** Biggest download we'll take for one file. */
const LIMIT = 5 * 1024 * 1024;

export type Where =
  | { kind: "yaml"; url: string }
  | { kind: "zip"; url: string }
  | { kind: "gist"; id: string };

/** Works out what a pasted link points at. Throws on things we can't use. */
export function resolve(input: string): Where {
  let u: URL;
  try {
    u = new URL(input.trim());
  } catch {
    throw new Error("That isn't a web address.");
  }
  if (u.protocol !== "https:" && !(u.protocol === "http:" && /^(localhost|127\.0\.0\.1)$/.test(u.hostname))) {
    throw new Error("Game links need to start with https://.");
  }
  const parts = u.pathname.split("/").filter(Boolean);
  if (u.hostname === "gist.github.com" || u.hostname === "gist.githubusercontent.com") {
    // gist.github.com/<user>/<id>, or a raw file in one.
    const id = parts.find((p) => /^[0-9a-f]{20,}$/i.test(p));
    if (!id) throw new Error("That gist link is missing the gist's id.");
    return { kind: "gist", id };
  }
  if (u.hostname === "github.com" && parts.length >= 2) {
    const [user, repo, mode, ...rest] = parts;
    const raw = (ref: string, path: string[]) =>
      `https://raw.githubusercontent.com/${user}/${repo}/${ref}/${path.join("/")}`.replace(/\/$/, "");
    if (!mode) return { kind: "yaml", url: raw("HEAD", ["game.yaml"]) };
    if ((mode === "blob" || mode === "raw") && rest.length >= 2) {
      const [ref, ...path] = rest;
      return /\.zip$/i.test(u.pathname) ? { kind: "zip", url: raw(ref, path) } : { kind: "yaml", url: raw(ref, path) };
    }
    if (mode === "tree" && rest.length >= 1) {
      const [ref, ...path] = rest;
      return { kind: "yaml", url: raw(ref, [...path, "game.yaml"]) };
    }
    throw new Error("That GitHub link doesn't point at a game. Link to the game's folder or its game.yaml.");
  }
  if (/\.zip$/i.test(u.pathname)) return { kind: "zip", url: u.href };
  if (/\.ya?ml$/i.test(u.pathname)) return { kind: "yaml", url: u.href };
  // A folder: its game.yaml.
  return { kind: "yaml", url: new URL("game.yaml", u.href.endsWith("/") ? u.href : `${u.href}/`).href };
}

async function get(url: string): Promise<Response> {
  let r: Response;
  try {
    r = await fetch(url, { credentials: "omit", referrerPolicy: "no-referrer" });
  } catch {
    const host = new URL(url).host;
    throw new Error(
      `The game couldn't be fetched from ${host}. That site may not let other sites read its files; GitHub, gists and most static hosts do.`,
    );
  }
  if (!r.ok) throw new Error(`${r.status === 404 ? "Nothing was found" : `The server said ${r.status}`} at ${url}.`);
  const size = Number(r.headers.get("content-length") ?? 0);
  if (size > LIMIT) throw new Error("That file is too big to be a game (over 5 MB).");
  return r;
}

async function text(url: string): Promise<string> {
  const t = await (await get(url)).text();
  if (t.length > LIMIT) throw new Error("That file is too big to be a game (over 5 MB).");
  return t;
}

/** The `scripts:` a game.yaml lists, or [] if it can't be read (the compiler will say why). */
async function scriptsOf(yamlText: string): Promise<string[]> {
  const { parse } = await import("yaml");
  try {
    const s = parse(yamlText)?.scripts;
    return Array.isArray(s) ? s.filter((p): p is string => typeof p === "string") : [];
  } catch {
    return [];
  }
}

/** Safe project paths only: no absolute paths, `..` or backslashes (the compiler refuses them too). */
const safe = (p: string) => !p.startsWith("/") && !p.includes("\\") && !p.split("/").includes("..");

/** Fetches a game from a link, as a project. */
export async function fetchGame(input: string): Promise<Files> {
  const where = resolve(input);
  if (where.kind === "zip") {
    const bytes = new Uint8Array(await (await get(where.url)).arrayBuffer());
    if (bytes.length > LIMIT) throw new Error("That file is too big to be a game (over 5 MB).");
    const files = fromZipBytes(bytes);
    if (typeof files === "string") throw new Error(files);
    return files;
  }
  if (where.kind === "gist") {
    const r = await get(`https://api.github.com/gists/${where.id}`);
    const gist = (await r.json()) as { files: Record<string, { content?: string; truncated?: boolean; raw_url: string }> };
    const byName = new Map<string, string>();
    for (const [name, f] of Object.entries(gist.files ?? {})) {
      byName.set(name, f.truncated || f.content === undefined ? await text(f.raw_url) : f.content);
    }
    const entry = byName.get("game.yaml") ?? byName.get("game.yml") ?? [...byName].find(([n]) => /\.ya?ml$/i.test(n))?.[1];
    if (entry === undefined) throw new Error("That gist has no game.yaml in it.");
    const files: Files = { "game.yaml": entry };
    // Gists are flat: scripts/camp.lua is found as camp.lua.
    for (const p of await scriptsOf(entry)) {
      const t = byName.get(p) ?? byName.get(p.split("/").pop() ?? p);
      if (t !== undefined && safe(p)) files[p] = t;
    }
    return files;
  }
  const yamlText = await text(where.url);
  const files: Files = { "game.yaml": yamlText };
  const scripts = (await scriptsOf(yamlText)).filter(safe);
  await Promise.all(
    scripts.map(async (p) => {
      files[p] = await text(new URL(p, where.url).href);
    }),
  );
  return files;
}
