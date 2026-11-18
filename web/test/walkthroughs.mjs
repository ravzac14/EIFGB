// Plays every example game's walkthrough through the WebAssembly build,
// and checks each one ends in a win. Run: node web/test/walkthroughs.mjs
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { loadEngine } from "../src/engine.ts";

const root = fileURLToPath(new URL("../../", import.meta.url));
const examples = join(root, "spec/examples");

function project(dir) {
  const files = {};
  const walk = (d) => {
    for (const name of readdirSync(d)) {
      const p = join(d, name);
      if (statSync(p).isDirectory()) walk(p);
      else if (/\.(ya?ml|lua)$/.test(name)) files[relative(dir, p).split("\\").join("/")] = readFileSync(p, "utf8");
    }
  };
  walk(dir);
  return files;
}

const engine = await loadEngine(new URL("../public/wasm/", import.meta.url));
let failed = 0;
for (const name of readdirSync(examples)) {
  const dir = join(examples, name);
  const started = engine.load(project(dir), "game.yaml", 1);
  const errors = started.diagnostics.filter((d) => d.severity === "error");
  if (!started.output) {
    console.log(`FAIL ${name}: didn't compile`, errors);
    failed++;
    continue;
  }
  const steps = readFileSync(join(dir, "walkthrough.txt"), "utf8")
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l && !l.startsWith("#"));
  let out = started.output;
  let midSave = null;
  for (const [i, step] of steps.entries()) {
    out = engine.input(step);
    if (i === Math.floor(steps.length / 2)) midSave = engine.save();
    if (out.ended) break;
  }
  // Saves round-trip: restore halfway and the game carries on.
  const restored = midSave ? engine.restore(midSave) : null;
  const restoredOk = !restored || restored.lines.some((l) => l.kind === "notice");
  const won = out.ended === "win";
  console.log(`${won && restoredOk ? "ok  " : "FAIL"} ${name}: ended ${out.ended ?? "never"}, ${steps.length} steps, score ${out.status.score}`);
  if (!won || !restoredOk) failed++;
}
process.exit(failed ? 1 : 0);
