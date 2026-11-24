// The editor's dictionary for its typo check: SCOWL's English words, US
// and British spellings both, to the size spell checkers use (60), into
// public/words.txt, one a line, the commonest first (so a suggestion can be
// the likeliest word). The editor fetches it only when it checks spelling.

import { createRequire } from "node:module";
import { copyFileSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const lists = require("wordlist-english");
const root = join(dirname(fileURLToPath(import.meta.url)), "..");

const seen = new Set();
const words = [];
for (const size of [10, 20, 35, 40, 50, 55, 60]) {
  for (const dialect of ["", "american/", "british/", "canadian/", "australian/"]) {
    for (const w of lists[`english/${dialect}${size}`] ?? []) {
      if (!/^[a-z]+$/.test(w) || seen.has(w)) continue;
      seen.add(w);
      words.push(w);
    }
  }
}
mkdirSync(join(root, "public"), { recursive: true });
writeFileSync(join(root, "public/words.txt"), `${words.join("\n")}\n`);
// Its licence asks for the notice to go with every copy.
copyFileSync(require.resolve("wordlist-english/Copyright"), join(root, "public/words-copyright.txt"));
console.log(`wrote ${words.length} words to public/words.txt`);
