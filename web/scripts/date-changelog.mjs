// Dates the changelog's new entries. New entries go under "## Unreleased"
// at the top of CHANGELOG.md; when the site is deployed (every push to
// main), this turns that heading into the day's date, or, if the day
// already has a section, moves the entries to the top of it. The deploy
// commits the result back, so the changelog never says "Unreleased" on the
// site, or for long in the repository.
//
// Run: node scripts/date-changelog.mjs [path] [YYYY-MM-DD]
// (default ../CHANGELOG.md and today, in UTC). Prints what it did.

import { readFileSync, writeFileSync } from "node:fs";
import { pathToFileURL, fileURLToPath } from "node:url";

/** The changelog with its Unreleased entries dated `date`, and what was done. */
export function dateChangelog(text, date) {
  const lines = text.split("\n");
  const start = lines.findIndex((l) => /^## Unreleased\s*$/i.test(l));
  if (start < 0) return { text, did: "nothing to date: there's no Unreleased section" };
  let end = lines.findIndex((l, i) => i > start && l.startsWith("## "));
  if (end < 0) end = lines.length;
  const body = lines.slice(start + 1, end);
  // Trim the blank lines around the entries.
  while (body.length && !body[0].trim()) body.shift();
  while (body.length && !body[body.length - 1].trim()) body.pop();
  if (!body.length) {
    // An empty heading: take it away.
    const rest = [...lines.slice(0, start), ...lines.slice(end)];
    return { text: rest.join("\n").replace(/\n{3,}/g, "\n\n"), did: "removed an empty Unreleased section" };
  }
  const next = lines[end] ?? "";
  if (next.startsWith(`## ${date}`)) {
    // The day has a section already (an earlier push today): add to its top.
    let at = end + 1;
    while (at < lines.length && !lines[at].trim()) at++;
    const merged = [...lines.slice(0, start), next, "", ...body, "", ...lines.slice(at)];
    return { text: merged.join("\n"), did: `added ${body.filter((l) => l.startsWith("- ")).length} entries to ${date}` };
  }
  const dated = [...lines.slice(0, start), `## ${date}`, "", ...body, "", ...lines.slice(end)];
  return { text: dated.join("\n").replace(/\n{3,}/g, "\n\n"), did: `dated the Unreleased entries ${date}` };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const path = process.argv[2] ?? fileURLToPath(new URL("../../CHANGELOG.md", import.meta.url));
  const date = process.argv[3] ?? new Date().toISOString().slice(0, 10);
  const before = readFileSync(path, "utf8");
  const { text, did } = dateChangelog(before, date);
  if (text !== before) writeFileSync(path, text);
  console.log(did);
}
