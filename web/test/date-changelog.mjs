// The changelog's Unreleased entries get the day's date when the site is
// deployed. Run: node test/date-changelog.mjs
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dateChangelog } from "../scripts/date-changelog.mjs";

const top = "# Changelog\n\nWhat's new, newest first.\n\n";
const older = "## 2026-11-20: notes\n\n- Notes.\n";

// A new day: the heading becomes the date.
assert.equal(
  dateChangelog(`${top}## Unreleased\n\n- Find in game.\n- History.\n\n${older}`, "2026-11-24").text,
  `${top}## 2026-11-24\n\n- Find in game.\n- History.\n\n${older}`,
);
// The day has a section already: the entries go to its top, keeping its title.
assert.equal(
  dateChangelog(`${top}## Unreleased\n\n- Tidy up.\n\n## 2026-11-24: editor tools\n\n- Find in game.\n\n${older}`, "2026-11-24").text,
  `${top}## 2026-11-24: editor tools\n\n- Tidy up.\n\n- Find in game.\n\n${older}`,
);
// An empty Unreleased heading is taken away; no heading, nothing changes.
assert.equal(dateChangelog(`${top}## Unreleased\n\n${older}`, "2026-11-24").text, `${top}${older}`);
assert.equal(dateChangelog(`${top}${older}`, "2026-11-24").text, `${top}${older}`);
// The real changelog has no Unreleased section left lying around in a dated one.
const real = readFileSync(new URL("../../CHANGELOG.md", import.meta.url), "utf8");
assert.ok((real.match(/^## Unreleased/gm) ?? []).length <= 1, "at most one Unreleased section");
assert.match(dateChangelog(real, "2099-01-01").text, /^## (?!Unreleased)/m);

console.log("ok   changelog dates");
