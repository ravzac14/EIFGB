# EIFGB on the web

The engine, compiler and Lua, compiled to WebAssembly, and the site that
uses them: a home page of games, and a player. The bridge is the
`eifgb-web` crate (`crates/eifgb-web`); `src/engine.ts` wraps it.

## Setting up (once)

```sh
rustup target add wasm32-unknown-emscripten
git clone https://github.com/emscripten-core/emsdk.git ~/emsdk
~/emsdk/emsdk install latest && ~/emsdk/emsdk activate latest
cd web && npm install
```

## Building and trying it

```sh
npm run wasm      # builds the engine into public/wasm/ (not committed)
npm test          # file loading, every example played to a win, and dev-server page paths
npm run dev       # a local server with live reload; open the address it prints
npm run build     # the finished site, in dist/
```

`npm run dev` and `npm run build` first copy the example games from
`spec/examples` into `public/games/`, and build the docs into
`public/docs/` (neither is committed). `npm run docs` rebuilds just the
docs.

## Docs

`scripts/build-docs.mjs` turns `README.md` (the guide), `docs/SCRIPTING.md`,
`docs/ADVANCED.md` and `spec/FORMAT.md` into pages under
`docs/`, with a contents sidebar, and makes a page per example game showing
its files with a Play button. Links between the documents become links
between the pages, and links to `spec/examples/<id>/` go to that game's
page. The build fails if a link points at a heading that doesn't exist, or
at a file that has no page. Code is highlighted at build time; the pages
need no JavaScript.

## Deploying

`.github/workflows/pages.yml` builds the engine to WebAssembly, runs
`npm test`, builds the site and publishes `web/dist` to GitHub Pages on
every push to `main` (Pages' source is set to *GitHub Actions*).
`.github/workflows/ci.yml` runs the Rust tests and clippy.

## Pages

- `index.html` (`src/home.ts`): the bundled games, and *Play your own
  game*: drop or choose a folder, a zip, or a `game.yaml`. Loaded games are
  kept in this browser and listed there.
- `play.html?example=<id>` or `play.html?local=<key>` (`src/play.ts`): the
  player. Choices show as buttons; up and down arrows recall commands.
  Games with mistakes show the same errors as `eifgb check`.

## The editor

`editor.html` (`src/editor/`) draws a game's map and writes every change
into its `game.yaml`. It covers rooms, exits and doors, objects and
containers, characters and their sides (factions), and scripts, timers,
endings, conversations, custom verbs, variables and script files.

- `model.ts`: the game file as a YAML document (the `yaml` library), with
  the edits the editor makes: rooms, exits, doors, renaming an ID along
  with everything that refers to it. Lines an edit doesn't touch are kept
  exactly as the author wrote them (`carryOver`).
- `layout.ts`: where rooms sit. Positions the author drags are kept in
  `map.yaml` beside `game.yaml` (the game never reads it); other rooms are
  placed by compass direction from the start room.
- `map.ts`: the SVG map. Click a room's + to add a room that way, drag a +
  onto a room to connect them, drag rooms and the background, scroll to
  zoom. Lines are one per route (an arrow when it's one way), with doors on
  them.
- `inspector.ts`: the details panel, for the game, a room, a way out, a
  door and a thing. A room lists what's in it as a tree: drag a thing into
  a container or onto a surface, or onto another room on the map. A
  thing's form puts its traits in plain words ("Can the player pick it
  up?", "Opens and closes", "Hidden until the player finds it" with what
  reveals it). An ID made from a name follows the name until it's set by
  hand. Characters show on the map as pills under their room, tinted by
  side and marked ⚔ when they'll attack the player; drag a pill to another
  room. A character's form covers what they do (each behaviour in plain
  words, with what it needs: how often they wander, which rooms, a shop's
  money and haggling), conversation, side, fighting, what they carry (with
  prices for a shopkeeper's stock) and more. Sides are listed in the game
  panel; a side's form sets who it attacks on sight and its habits.
- `story.ts`: scripts and the story's moving parts. Rooms, things,
  characters and the game have a Scripts section: each handler with a
  plain-words label ("Before TAKE", "When the player comes in"), its Lua,
  and its problems under it; add one by choosing when (an event, or
  before / instead of / after a verb). The game panel lists endings,
  timers, conversations and menus, your own verbs, variables and script
  files, each with its own form. A conversation is edited as parts, each
  with what's said and the player's replies, and where each reply leads.
- `ui.ts`: the shared form builders, and the context every panel gets.
- `main.ts`: projects (kept with *your games* in the browser), undo and
  redo, live checking with the WebAssembly compiler (click a problem to go
  to it), the *Game file* tab (edit the YAML; the map follows), Play (from
  the start, in a new tab) and Download (a zip). Undo covers every file. The
*Files* tab edits `game.yaml` or any script file. Problems refresh the
details panel when the cursor isn't in it, so typing is never
interrupted.

`test/editor-model.mjs` checks the model against the examples (untouched
files round-trip exactly, and edited ones compile), and
`test/editor-ui.mjs` drives the page in a simulated browser (happy-dom).

## Sharing

- `play.html?game=<url>` plays a game from a link (`src/remote.ts`): a raw
  `game.yaml` (its `scripts:` are fetched from beside it), a folder, a
  zip, a GitHub repository, folder or file page, or a gist.
- `gallery.html` (`src/gallery.ts`) lists `gallery.yaml`, with search and
  tags, and *Share your game* checks a link and gives its play link.
  `scripts/bundle-gallery.mjs` checks every entry at build time.
- `docs/SHARING.md` (the Sharing docs page) is the guide for authors.

## Saves

Everything is kept in the browser's local storage (`src/store.ts`):

- **Autosave:** after every command. Coming back to a game carries on
  where you were; *Restart* begins again. Finishing a game clears it.
- **Slots:** typing SAVE, or *Saves → Save*, keeps a named save; RESTORE
  opens the Saves dialog.
- **Files:** any slot can be downloaded as a `.eifgb-save.json` file, and
  loaded back on another device (*Load a save file…*).

## The engine API (`src/engine.ts`)

- `loadEngine(base)`: loads the WebAssembly from the folder at `base`.
- `engine.load(files, entry, seed)`: compiles `files` (`{path: text}`) and
  starts the game: `{ diagnostics, meta?, output? }`.
- `engine.check(files, entry)`: compiles only: `{ diagnostics }`.
- `engine.input(line)`: one command: the output (`lines`, `status`,
  `options`, `ended`, `save`, `restore_requested`, `quit_requested`).
- `engine.save()` / `engine.restore(save)`: the whole game state as a
  string, and putting it back.
