# Changelog

What's new, newest first. EIFGB is still early (version 0.1), and the game
file format may change before 1.0; anything that changes how existing games
behave will be called out here.

## Unreleased

- Light shows through open ways between rooms: in a dark room next to a
  lit one, "Light spills in from the north."; in a lit room next to a dark
  one, "It's dark to the north."

- The home page's heading is now *Interactive Fiction Game Builder/Player*.
- This changelog, on the website.
- A tiny game to try at the top of the home page: you're locked in a study.
- New looks in *Text and colours*: Terminal (green or amber, with a soft
  glow and scan lines) and High contrast. A *Very legible* typeface
  (Atkinson Hyperlegible), roomier line spacing, and narrow, medium or wide
  columns. They apply to the player, the editor and the docs.
- The site has an icon, and links to it show a preview card when shared.
- A map in the player that fills in as you explore: the places you've been,
  the ways between them, doors, and the ways out you haven't tried yet.
  Rooms sit where the author put them in the editor.
- The player scrolls down to show each reply as it arrives; a long reply
  stops at its first line, so you can read it from the beginning.
- Choices (in conversations and menus) are typed by number, like every
  other command, rather than clicked.
- The guide's list of commands every game understands is now a set of
  tables, with the other ways to say each one.

## 2026-11-17: the website

Everything now runs in the browser, at
[ravzac14.github.io/EIFGB](https://ravzac14.github.io/EIFGB/).

### Playing

- Play the example games, or your own: drop a game folder, a zip or a
  `game.yaml` on the home page.
- Your place is saved automatically after every command. SAVE and RESTORE
  keep named saves, and saves can be downloaded and loaded on another device.
- A book-like page with light, dark and sepia colours, three text sizes, and
  a layout that works on phones.

### Sharing

- Share a game with a link: `play.html?game=` and the address of a game
  hosted on GitHub, in a gist, or anywhere public.
- A [gallery](https://ravzac14.github.io/EIFGB/gallery.html) of games, which
  you can add yours to with a pull request. See
  [Sharing your game](docs/SHARING.md).
- Other people's games run sealed off from your computer. The sharing guide
  explains what that means, and what a game's host can see.

### Making games: the visual editor

- Draw your map: rooms, the ways between them, doors (locked, secret).
- Put things in rooms, in containers and on surfaces by dragging them.
- Add characters (people to talk to, shopkeepers, enemies, guards,
  companions) and sides that fight each other.
- Write scripts, timers, endings, conversations, your own verbs and
  variables, with problems shown as you type.
- Everything you do is written into your game's file, which you can also
  edit by hand. Download it as a zip at any time.

### Docs

- The guide, both tutorials and the format reference are pages on the site,
  with a page for each example game showing its files.

### Checking games

- `eifgb check` (and the editor) now find Lua syntax mistakes in every
  script, not just in script files, before the game is played.

## 2026-11-17: characters, and tricks players can find

### Characters

- People and creatures that stay put, talk, wander, trade, hunt, guard,
  follow or defend themselves. See
  [Characters](README.md#characters-npcs-and-enemies).
- Fighting, with health and damage; buying and selling; haggling.
- Sides (factions) that hate each other, eat what's left lying around, and
  turn on the player if provoked in front of them. Patience, apologies,
  pacifists and characters who can't be hurt.

### Tricks players can find

- Disguises, bait, hiding, and bodies that tell how they died.
- Making noise, mimicking calls that only one side answers, and eavesdropping
  through walls.
- Taming, gifts and bribes; throwing things, and companions that fetch;
  naming things (CALL THE DOG REX); giving orders.
- Tracks to read and follow; witnesses; hiding bodies.
- Sleep and sneaking; peeking into the next room; full darkness and light.
- Pickpocketing; fear and morale; traps.
- Fire that spreads, scorches and can be put out; smashing things.

### Tutorials

- The scripting tutorial (Campfire) and the advanced tutorial (Blackfang
  Pass).

## 2026-11-16: the first version

- The game file format: rooms, exits, objects and characters in YAML, with
  Lua for anything special.
- The compiler (`eifgb check`, `eifgb build`), the engine, and
  `eifgb play` to play in a terminal.
- A forgiving parser, undo, saving, menus and dialogue, endings and score.
- Descriptions that mention what's open, locked, empty or worn, without the
  author writing it.
- Doors that work from both sides, hidden things and secret doors,
  containers and surfaces, carrying limits, and timers.
- The README tutorial (Midnight Snack) and the Lighthouse sampler.
