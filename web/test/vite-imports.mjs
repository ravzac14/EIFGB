// Lets Node load page code the way Vite does: `import "x.css"` does nothing,
// and `import text from "x?raw"` gives the file's text. Import this first.
import { readFileSync } from "node:fs";
import { registerHooks } from "node:module";
import { fileURLToPath } from "node:url";

registerHooks({
  resolve(spec, ctx, next) {
    if (spec.endsWith("?raw")) return { url: new URL(spec, ctx.parentURL).href, shortCircuit: true };
    return next(spec, ctx);
  },
  load(url, ctx, next) {
    if (url.endsWith("?raw")) {
      const text = readFileSync(fileURLToPath(url.slice(0, -4)), "utf8");
      return { format: "module", source: `export default ${JSON.stringify(text)};`, shortCircuit: true };
    }
    if (url.endsWith(".css")) return { format: "module", source: "", shortCircuit: true };
    return next(url, ctx);
  },
});
