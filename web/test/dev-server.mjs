// Starts the Vite dev server briefly and checks that each page path serves
// the right page (and that wrong paths don't quietly serve the home page).
// Run after `npm run prepare-site`: node test/dev-server.mjs
import assert from "node:assert/strict";
import { createServer } from "vite";
import { fileURLToPath } from "node:url";

const server = await createServer({ root: fileURLToPath(new URL("..", import.meta.url)), server: { port: 5199, strictPort: true }, logLevel: "silent" });
await server.listen();
const title = async (p) => {
  const r = await fetch(`http://localhost:5199${p}`);
  return `${r.status} ${(await r.text()).match(/<title>([^<]*)<\/title>/)?.[1] ?? ""}`;
};
try {
  const results = {};
  for (const p of ["/", "/play.html", "/gallery.html", "/docs/", "/docs", "/docs/scripting.html", "/docs/docs/"]) results[p] = await title(p);
  console.log(results);
  assert.equal(results["/"], "200 EIFGB Games");
  assert.equal(results["/gallery.html"], "200 Gallery");
  assert.match(results["/docs/"], /^200 Guide · EIFGB$/);
  assert.match(results["/docs"], /^200 Guide · EIFGB$/);
  // Without the slash, it redirects, so the docs' relative links work.
  assert.match((await fetch("http://localhost:5199/docs")).url, /\/docs\/$/);
  assert.equal(await title("/play.html?example=campfire"), "200 Play");
  assert.match(results["/docs/scripting.html"], /^200 Scripting tutorial/);
  assert.match(results["/docs/docs/"], /^404/);
  console.log("ok   dev server");
} finally {
  await server.close();
}
