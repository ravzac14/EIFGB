import { defineConfig } from "vite";
import { resolve } from "node:path";

// Relative paths everywhere, so the site works from any folder: a GitHub
// Pages project site (/<repo>/), a custom domain, or a local server.
export default defineConfig({
  base: "./",
  build: {
    target: "es2022",
    rollupOptions: {
      input: {
        home: resolve(import.meta.dirname, "index.html"),
        play: resolve(import.meta.dirname, "play.html"),
      },
    },
  },
});
