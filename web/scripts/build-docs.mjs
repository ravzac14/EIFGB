// Builds the documentation pages into public/docs/: the README, the
// tutorials, the format reference, the guide for AI assistants and the
// changelog, plus a page for each example game showing its files. Links
// between the documents are rewritten to the pages, and every link to a
// heading is checked.
//
// For AI assistants and other tools, every page is also written as Markdown
// beside its HTML (index.md…), and the site root gets llms.txt (what's
// here, and where), llms-full.txt (all of it in one file) and sitemap.xml.
import { copyFileSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { join, posix, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { Marked } from "marked";
import GithubSlugger from "github-slugger";
import hljs from "highlight.js/lib/core";
import yaml from "highlight.js/lib/languages/yaml";
import lua from "highlight.js/lib/languages/lua";
import bash from "highlight.js/lib/languages/bash";

hljs.registerLanguage("yaml", yaml);
hljs.registerLanguage("lua", lua);
hljs.registerLanguage("sh", bash);

const root = fileURLToPath(new URL("../../", import.meta.url));
const web = fileURLToPath(new URL("../", import.meta.url));
const out = join(web, "public/docs");

/** The documents, in reading order: source path → page. */
const PAGES = [
  { src: "README.md", page: "index.html", nav: "Guide", about: "The guide: a step-by-step tutorial (Midnight Snack), states, containers, timers, characters, the commands every game understands, and a cheat sheet." },
  { src: "docs/SCRIPTING.md", page: "scripting.html", nav: "Scripting", about: "Tutorial: Lua scripting, handlers, state, timers, darkness and a character to talk to (Campfire)." },
  { src: "docs/ADVANCED.md", page: "advanced.html", nav: "Advanced", about: "Tutorial: factions, hunters, corpses and scavengers, a guard, and other ways through (Blackfang Pass)." },
  { src: "spec/FORMAT.md", page: "format.html", nav: "Format reference", about: "The source format in full: every section, field, rule and message key." },
  { src: "docs/SHARING.md", page: "sharing.html", nav: "Sharing", about: "Sharing games by link, the gallery, and what running someone else's game can and can't do." },
  { src: "docs/AI.md", page: "ai.html", nav: "For AI", about: "For language models and coding agents: the format in brief, the rules that trip up first drafts, a complete checked example, and how to check and play a game." },
  { src: "CHANGELOG.md", page: "changelog.html", nav: "Changelog", about: "What's new, newest first." },
];
const EXAMPLES = {
  midnight_snack: "The game from the README tutorial.",
  lighthouse: "A sampler of every feature.",
  campfire: "The game from the scripting tutorial.",
  blackfang: "The game from the advanced tutorial.",
};

const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
const unesc = (s) => s.replace(/&(amp|lt|gt|quot|#39);/g, (_, e) => ({ amp: "&", lt: "<", gt: ">", quot: '"', "#39": "'" })[e]);
const plain = (html) => unesc(html.replace(/<[^>]+>/g, ""));

function highlight(code, lang) {
  const l = { yml: "yaml", bash: "sh", shell: "sh" }[lang] ?? lang;
  if (l && hljs.getLanguage(l)) return `<pre class="code ${l}"><code>${hljs.highlight(code, { language: l }).value}</code></pre>`;
  // Unlabelled blocks are mostly play transcripts: colour the commands.
  const lines = esc(code)
    .split("\n")
    .map((line) => (/^&gt; ?/.test(line) ? `<span class="cmd">${line}</span>` : line));
  return `<pre class="code plain"><code>${lines.join("\n")}</code></pre>`;
}

/** Where a link in `from` (a repo path) goes on the site, or null if nowhere. */
function rewrite(href, from) {
  if (/^([a-z]+:|\/\/)/i.test(href)) return { href, external: true };
  if (href.startsWith("#")) return { href, page: PAGES.find((p) => p.src === from).page, hash: href.slice(1) };
  const [path, hash = ""] = href.split("#");
  const target = posix.normalize(posix.join(posix.dirname(from), path));
  const page = PAGES.find((p) => p.src === target);
  if (page) return { href: page.page + (hash ? `#${hash}` : ""), page: page.page, hash };
  // The site's own pages and files (web/gallery.html → ../gallery.html).
  const site = target.match(/^web\/([\w-]+\.(?:html|txt|xml))$/);
  if (site) return { href: `../${site[1]}` };
  const ex = target.match(/^spec\/examples\/([\w-]+)(\/|$)/);
  if (ex) return { href: `example-${ex[1]}.html` };
  return null;
}

const NAV = (current) =>
  PAGES.map((p) => `<a href="${p.page}"${p.page === current ? ' aria-current="page"' : ""}>${esc(p.nav)}</a>`).join("") +
  `<a href="examples.html"${current.startsWith("example") ? ' aria-current="page"' : ""}>Examples</a>`;

// Applies the player's "Text and colours" choices before the page paints.
const THEME = `<script>try{var s=JSON.parse(localStorage.getItem("eifgb:settings")||"{}"),h=document.documentElement;if(s.theme&&s.theme!=="auto")h.dataset.theme=s.theme;["size","face","spacing","width"].forEach(function(k){if(s[k])h.dataset[k]=s[k]})}catch(e){}</script>`;

/** Where the site lives, for link previews (which need full addresses). */
const SITE = "https://ravzac14.github.io/EIFGB/";
const preview = (title, page) => `<meta property="og:type" content="website">
<meta property="og:site_name" content="Interactive Fiction Game Builder/Player">
<meta property="og:title" content="${esc(title)}">
<meta property="og:description" content="Make and play interactive fiction in your browser.">
<meta property="og:url" content="${SITE}docs/${page}">
<meta property="og:image" content="${SITE}og.png">
<meta name="twitter:card" content="summary_large_image">`;

function layout({ title, page, toc, body, foot = "" }) {
  const contents = toc.length
    ? `<aside class="toc"><details id="toc"><summary>On this page</summary><ul>${toc
        .map((h) => `<li class="h${h.depth}"><a href="#${h.id}">${h.html}</a></li>`)
        .join("")}</ul></details></aside>
<script>if(matchMedia("(min-width: 64rem)").matches)document.getElementById("toc").open=true</script>`
    : "";
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${esc(title)} · EIFGB</title>
${THEME}
<link rel="icon" href="../favicon.svg" type="image/svg+xml">
${preview(title, page)}
<link rel="alternate" type="text/markdown" href="${page.replace(/\.html$/, ".md")}" title="This page as Markdown">
<link rel="stylesheet" href="site.css">
</head>
<body>
<header class="bar">
  <a href="../" class="brand">EIFGB</a>
  <nav class="nav">${NAV(page)}</nav>
  <a href="../feedback.html?from=docs/${page}" class="feedback">Feedback</a>
  <a href="../" class="play">Play games</a>
</header>
<div class="layout${toc.length ? "" : " wide"}">
${contents}
<main class="prose">
${body}
${foot}
</main>
</div>
</body>
</html>
`;
}

// --------------------------------------------------------------- render

const links = []; // { from, href, page, hash } to check once every page is known
const anchors = new Map(); // page → Set of heading ids
const warnings = [];

function render(src, markdown, page) {
  const slugger = new GithubSlugger();
  const toc = [];
  const ids = new Set();
  anchors.set(page, ids);
  const marked = new Marked({ gfm: true });
  marked.use({
    renderer: {
      heading({ tokens, depth }) {
        const html = this.parser.parseInline(tokens);
        const id = slugger.slug(plain(html));
        ids.add(id);
        if (depth === 2 || depth === 3) toc.push({ depth, id, html: html.replace(/<\/?a[^>]*>/g, "") });
        return `<h${depth} id="${id}"><a class="anchor" href="#${id}" aria-hidden="true">#</a>${html}</h${depth}>\n`;
      },
      code({ text, lang }) {
        return highlight(text, (lang ?? "").trim().split(/\s/)[0]);
      },
      link({ href, title, tokens }) {
        const text = this.parser.parseInline(tokens);
        const r = rewrite(href, src);
        if (!r) {
          warnings.push(`${src}: link to ${href} has nowhere to go on the site; shown as plain text`);
          return text;
        }
        if (r.page) links.push({ from: src, href, page: r.page, hash: r.hash });
        const t = title ? ` title="${esc(title)}"` : "";
        return `<a href="${esc(r.href)}"${t}${r.external ? ' rel="noopener"' : ""}>${text}</a>`;
      },
      blockquote({ tokens }) {
        // GitHub's alerts: > [!NOTE], > [!WARNING]…
        const html = this.parser.parse(tokens);
        const alert = html.match(/^<p>\[!(NOTE|TIP|IMPORTANT|WARNING|CAUTION)\]\s*/);
        if (!alert) return `<blockquote>${html}</blockquote>\n`;
        const kind = alert[1].toLowerCase();
        const label = { note: "Note", tip: "Tip", important: "Important", warning: "Warning", caution: "Caution" }[kind];
        return `<div class="callout ${kind}" role="note"><p class="callout-label">${label}</p><p>${html.slice(alert[0].length)}</div>\n`;
      },
      table(token) {
        // Wide tables scroll on their own, not the page.
        return `<div class="table">${marked.Renderer.prototype.table.call(this, token)}</div>`;
      },
    },
  });
  const body = marked.parse(markdown);
  const title = plain(body.match(/<h1[^>]*>(.*?)<\/h1>/s)?.[1] ?? page).replace(/^#/, "");
  return { body, toc, title };
}

mkdirSync(out, { recursive: true });

PAGES.forEach((p, i) => {
  const md = readFileSync(join(root, p.src), "utf8");
  const r = render(p.src, md, p.page);
  const { body, toc } = r;
  // The README's heading is just "EIFGB": name the tab after its menu entry.
  const title = r.title === "EIFGB" ? p.nav : r.title;
  const prev = PAGES[i - 1];
  const next = PAGES[i + 1];
  const foot = `<nav class="pager">${prev ? `<a href="${prev.page}">← ${esc(prev.nav)}</a>` : "<span></span>"}${
    next ? `<a href="${next.page}">${esc(next.nav)} →</a>` : `<a href="examples.html">Examples →</a>`
  }</nav>`;
  writeFileSync(join(out, p.page), layout({ title, page: p.page, toc, body, foot }));
});

// ------------------------------------------------------------- examples

function files(dir) {
  const found = [];
  const walk = (d) => {
    for (const name of readdirSync(d).sort()) {
      const p = join(d, name);
      if (statSync(p).isDirectory()) walk(p);
      else found.push(relative(dir, p).split("\\").join("/"));
    }
  };
  walk(dir);
  // game.yaml first, then scripts, walkthrough last.
  const rank = (f) => (f === "game.yaml" ? 0 : f === "walkthrough.txt" ? 2 : 1);
  return found.sort((a, b) => rank(a) - rank(b) || a.localeCompare(b));
}

const examplesDir = join(root, "spec/examples");
const list = [];
for (const id of readdirSync(examplesDir).sort((a, b) => Object.keys(EXAMPLES).indexOf(a) - Object.keys(EXAMPLES).indexOf(b))) {
  const dir = join(examplesDir, id);
  if (!statSync(join(dir, "game.yaml"), { throwIfNoEntry: false })) continue;
  const fs = files(dir);
  const yamlText = readFileSync(join(dir, "game.yaml"), "utf8");
  const title = yamlText.match(/^\s*title:\s*"?([^"\n]+)"?/m)?.[1] ?? id;
  const about = EXAMPLES[id] ?? "";
  list.push({ id, title, about, files: fs.map((f) => ({ name: f, text: readFileSync(join(dir, f), "utf8") })) });
  const sections = fs
    .map((f) => {
      const text = readFileSync(join(dir, f), "utf8");
      const lang = f.endsWith(".lua") ? "lua" : /\.ya?ml$/.test(f) ? "yaml" : "";
      const id = f.replace(/[^\w]+/g, "-");
      if (f === "walkthrough.txt") {
        return `<h2 id="${id}">Walkthrough</h2><details><summary>Show the solution (spoilers)</summary>${highlight(text.trimEnd(), "")}</details>`;
      }
      return `<h2 id="${id}"><code>${esc(f)}</code></h2>${highlight(text.trimEnd(), lang)}`;
    })
    .join("\n");
  const body = `<h1>${esc(title)}</h1>
<p>${esc(about)} These are the game's files, exactly as written. <code>spec/examples/${esc(id)}/</code> in the repository.</p>
<p class="actions"><a class="button primary" href="../play.html?example=${encodeURIComponent(id)}">Play ${esc(title)}</a></p>
${sections}`;
  const toc = fs.map((f) => ({ depth: 2, id: f.replace(/[^\w]+/g, "-"), html: f === "walkthrough.txt" ? "Walkthrough" : esc(f) }));
  writeFileSync(join(out, `example-${id}.html`), layout({ title, page: `example-${id}.html`, toc, body }));
}
writeFileSync(
  join(out, "examples.html"),
  layout({
    title: "Example games",
    page: "examples.html",
    toc: [],
    body: `<h1>Example games</h1>
<p>Each one is a complete game you can play, and read: every file is shown as written.</p>
<ul class="cards">${list
      .map(
        (g) =>
          `<li><a href="example-${g.id}.html"><strong>${esc(g.title)}</strong></a> ${esc(g.about)}<br><a href="../play.html?example=${encodeURIComponent(g.id)}">Play</a> · <a href="example-${g.id}.html">Read the files</a></li>`,
      )
      .join("")}</ul>`,
  }),
);

// ------------------------------------------------- for AI assistants

const REPO = "https://github.com/ravzac14/EIFGB/blob/main/";
const mdName = (page) => page.replace(/\.html$/, ".md");

/** A link in Markdown copied from `from`: to the other Markdown pages, or to the repository. */
function mdLink(href, from) {
  const r = rewrite(href, from);
  if (!r) return REPO + posix.normalize(posix.join(posix.dirname(from), href));
  if (r.external) return href;
  if (r.page) return mdName(r.page) + (r.hash ? `#${r.hash}` : "");
  return r.href.replace(/^(example-[\w-]+)\.html$/, "$1.md");
}

/** Rewrites a Markdown document's links, leaving code blocks alone. */
function mdLinks(markdown, from) {
  let fence = false;
  return markdown
    .split("\n")
    .map((line) => {
      if (/^\s*(```|~~~)/.test(line)) fence = !fence;
      if (fence) return line;
      return line.replace(/\]\(([^)\s]+)\)/g, (_, href) => `](${mdLink(href, from)})`);
    })
    .join("\n");
}

const markdownPages = PAGES.map((p) => {
  const text = mdLinks(readFileSync(join(root, p.src), "utf8"), p.src);
  writeFileSync(join(out, mdName(p.page)), text);
  return { ...p, md: mdName(p.page), text };
});
const fence = (f) => (f.endsWith(".lua") ? "lua" : /\.ya?ml$/.test(f) ? "yaml" : "text");
const exampleMarkdown = list.map((g) => {
  const text = `# ${g.title}

${g.about} The game's files, exactly as written (\`spec/examples/${g.id}/\` in the repository). Play it at ${SITE}play.html?example=${g.id}

${g.files
  .map((f) => `## ${f.name === "walkthrough.txt" ? "Walkthrough (the solution)" : `\`${f.name}\``}\n\n\`\`\`${fence(f.name)}\n${f.text.trimEnd()}\n\`\`\``)
  .join("\n\n")}
`;
  writeFileSync(join(out, `example-${g.id}.md`), text);
  return { ...g, md: `example-${g.id}.md`, text };
});
writeFileSync(
  join(out, "examples.md"),
  `# Example games\n\n${exampleMarkdown.map((g) => `- [${g.title}](${g.md}): ${g.about}`).join("\n")}\n`,
);

// llms.txt (https://llmstxt.org): what this is, and where everything is.
const pageLine = (p) => `- [${p.page === "ai.html" ? "Writing games with an AI assistant" : p.nav}](${SITE}docs/${p.md}): ${p.about}`;
const ai = markdownPages.find((p) => p.page === "ai.html");
const llms = `# Interactive Fiction Game Builder/Player (EIFGB)

> Make and play parser-based interactive fiction (text adventures). A game is a YAML file, with Lua for anything special; a Rust engine compiles and plays it, on the command line or in the browser (as WebAssembly) on this site, which also has a visual editor.

- A game is a \`game.yaml\` (plus any \`.lua\` files it lists). Start with the guide for AI assistants below: it has the format in brief and a complete, checked example.
- Check a game with \`eifgb check game.yaml\`, and play a list of commands with \`eifgb play game.yaml --commands walkthrough.txt\`. The \`eifgb\` command is built from the source: https://github.com/ravzac14/EIFGB
- Any public \`game.yaml\` (or a GitHub folder, gist or zip) plays in the browser at ${SITE}play.html?game=<its address>
- Every docs page here is Markdown at the addresses below (and HTML without the .md). All of them in one file: ${SITE}llms-full.txt

## Start here

${pageLine(ai)}

## Docs

${markdownPages.filter((p) => p !== ai && p.page !== "changelog.html").map(pageLine).join("\n")}

## Examples

${exampleMarkdown.map((g) => `- [${g.title}](${SITE}docs/${g.md}): ${g.about} Every file, and its walkthrough.`).join("\n")}

## Optional

${pageLine(markdownPages.find((p) => p.page === "changelog.html"))}
- [Source code](https://github.com/ravzac14/EIFGB): the engine, compiler, command-line tool and this website.
`;
const pub = join(web, "public");
writeFileSync(join(pub, "llms.txt"), llms);
const full = [ai, ...markdownPages.filter((p) => p !== ai)]
  .map((p) => `<!-- ${SITE}docs/${p.md} -->\n\n${p.text.trim()}`)
  .concat(exampleMarkdown.map((g) => `<!-- ${SITE}docs/${g.md} -->\n\n${g.text.trim()}`))
  .join("\n\n---\n\n");
writeFileSync(join(pub, "llms-full.txt"), `${full}\n`);

// sitemap.xml: every page worth finding.
const urls = ["", "gallery.html", "editor.html", ...PAGES.map((p) => `docs/${p.page}`), "docs/examples.html", ...list.map((g) => `docs/example-${g.id}.html`)];
writeFileSync(
  join(pub, "sitemap.xml"),
  `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${urls.map((u) => `  <url><loc>${SITE}${u}</loc></url>`).join("\n")}\n</urlset>\n`,
);

// ------------------------------------------------------------ checking

for (const l of links) {
  if (l.hash && !anchors.get(l.page)?.has(l.hash)) warnings.push(`${l.from}: link ${l.href} points at a heading that doesn't exist`);
}

// The legible typeface, served with the docs.
const fonts = join(web, "node_modules/@fontsource/atkinson-hyperlegible/files");
mkdirSync(join(out, "fonts"), { recursive: true });
const faces = [400, 700]
  .map((w) => {
    const file = `atkinson-hyperlegible-latin-${w}-normal.woff2`;
    copyFileSync(join(fonts, file), join(out, "fonts", file));
    return `@font-face { font-family: "Atkinson Hyperlegible"; font-style: normal; font-weight: ${w}; font-display: swap; src: url(fonts/${file}) format("woff2"); }`;
  })
  .join("\n");

// The stylesheet: the typeface, the site's shared look, and the docs' own.
writeFileSync(
  join(out, "site.css"),
  `${faces}\n${readFileSync(join(web, "src/style.css"), "utf8")}\n${readFileSync(join(web, "src/docs.css"), "utf8")}`,
);

for (const w of warnings) console.warn(`warning: ${w}`);
console.log(`built ${PAGES.length + list.length + 1} docs pages into public/docs/ (with Markdown copies), and llms.txt, llms-full.txt and sitemap.xml`);
if (warnings.length && process.argv.includes("--strict")) process.exit(1);
