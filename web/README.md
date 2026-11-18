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

## Pages

- `index.html` (`src/home.ts`): the bundled games, and *Play your own
  game*: drop or choose a folder, a zip, or a `game.yaml`. Loaded games are
  kept in this browser and listed there.
- `play.html?example=<id>` or `play.html?local=<key>` (`src/play.ts`): the
  player. Choices show as buttons; up and down arrows recall commands.
  Games with mistakes show the same errors as `eifgb check`.

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
