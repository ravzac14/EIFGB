import { defineConfig, type Plugin } from "vite";
import { execSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

/**
 * The dev server serves generated pages in public/ (the docs) the way a
 * static host does: `/docs/` is `/docs/index.html`, and `/docs` redirects
 * to `/docs/` so the pages' relative links resolve.
 */
function publicIndexes(): Plugin {
  return {
    name: "public-indexes",
    configureServer(server) {
      const pub = server.config.publicDir;
      server.middlewares.use((req, res, next) => {
        const [path, query = ""] = (req.url ?? "/").split("?");
        if (path === "/" || /\.\w+$/.test(path)) return next();
        const dir = decodeURIComponent(path).replace(/\/+$/, "");
        if (!existsSync(resolve(pub, `.${dir}`, "index.html"))) return next();
        if (!path.endsWith("/")) {
          res.statusCode = 301;
          res.setHeader("Location", `${path}/${query ? `?${query}` : ""}`);
          return res.end();
        }
        req.url = `${path}index.html${query ? `?${query}` : ""}`;
        next();
      });
    },
  };
}

/**
 * What build of the site this is, for feedback reports: the commit (from
 * GitHub Actions, or the local checkout), when it was built, and the
 * engine's version from Cargo.toml.
 */
function buildInfo() {
  let commit = process.env.GITHUB_SHA?.slice(0, 7) ?? "";
  if (!commit) {
    try {
      commit = execSync("git rev-parse --short HEAD", { stdio: ["ignore", "pipe", "ignore"] }).toString().trim();
      if (execSync("git status --porcelain", { stdio: ["ignore", "pipe", "ignore"] }).toString().trim()) commit += "+changes";
    } catch {
      commit = "unknown";
    }
  }
  const cargo = readFileSync(resolve(import.meta.dirname, "../Cargo.toml"), "utf8");
  const engine = cargo.match(/\[workspace\.package\][^[]*?version\s*=\s*"([^"]+)"/)?.[1] ?? "unknown";
  return { commit, built: new Date().toISOString().slice(0, 10), engine };
}

// Relative paths everywhere, so the site works from any folder: a GitHub
// Pages project site (/<repo>/), a custom domain, or a local server.
export default defineConfig({
  base: "./",
  // Separate pages, not a single-page app: a wrong path is a 404, not the
  // home page served at the wrong address.
  appType: "mpa",
  plugins: [publicIndexes()],
  define: { __BUILD__: JSON.stringify(buildInfo()) },
  build: {
    target: "es2022",
    rollupOptions: {
      input: {
        home: resolve(import.meta.dirname, "index.html"),
        play: resolve(import.meta.dirname, "play.html"),
        gallery: resolve(import.meta.dirname, "gallery.html"),
        editor: resolve(import.meta.dirname, "editor.html"),
        feedback: resolve(import.meta.dirname, "feedback.html"),
      },
    },
  },
});
