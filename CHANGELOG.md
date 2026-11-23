# Changelog

What's new, newest first. EIFGB is still early (version 0.1), and the game
file format may change before 1.0; anything that changes how existing games
behave will be called out here.

## Unreleased

- Help for players, in every game:
  - WHERE IS THE KEY (or FIND THE KEEPER) says where you last saw it, and
    GO TO THE KEEPER walks you there. It's a reminder, not a hint: it only
    knows what you've seen.
  - UNDO 3 takes back three moves at once.
  - OOPS LANTERN fixes the word the game didn't know in your last command,
    and a slip too close to more than one word asks "Did you mean TAKE?"
    instead of giving up.
  - BRIEF stops describing places you've already been; VERBOSE goes back.
  - RECAP sums up where things stand, for coming back to a game after a
    while. A restored game mentions it.
  - ASK THE KEEPER ABOUT THE GATE goes straight to the part of the
    conversation about the gate.
  - AGAIN WITH THE IRON KEY repeats your last command with something else.
- BRIEF is now a command, so it's no longer an unknown word.
- Fixed: FEEL FOR THE COIN in the dark no longer counts everything else
  there as found.
- Players can keep notes: NOTE THE CODE MIGHT BE 4512 writes one, word for
  word, with no turn taken. JOURNAL and NOTES list them; ERASE NOTE 2 and
  CLEAR NOTES rub them out. They're kept with saves, and UNDO and RESTART
  leave them alone.
- How things behave, with no scripts needed:
  - Flames: `needs_flame` lights (candles) light only from something
    burning; LIGHT THE CANDLE WITH THE MATCHES strikes the match first.
  - Fuel: `fuel` lights burn down, flicker, and go out, or are gone with
    `burns_away` (matches).
  - Liquids: `holds_liquid` and `liquid: water`; FILL, POUR, DRINK, and
    POUR THE WATER ON THE FIRE.
  - Wear: `uses` and `breaks`.
  - Codes: `code: 4512` on a lockable thing; TYPE 4512.
  - Writing: `text`, for READ.
  - Sounds and smells: `sound`, `smell`; LISTEN and SMELL reach next door.
  - Weather: a `weather:` section, `outdoors` rooms, `game.weather` and
    `set_weather`.
  - Timers can go off by `chance: 10%`.
  - Companions are known in the dark, and one that sees in the dark can
    fetch what the player can't see.
- READ is now its own verb (it was a synonym for EXAMINE); LISTEN and SMELL
  are built in. Games' handlers for them still come first.
- Scripts can light and darken rooms: `world.cellar.dark = false`. When
  the light changes from a script or timer (or a lamp running dry), the
  room is described at once, or the darkness is. Scenery that's only a
  light (a light source with no description) isn't something to feel for.
- A timer can be heard only in some places: `heard_in: tower` (a room, a
  list of them, or a thing such as a clock, heard wherever it is).
  Elsewhere it goes off unannounced. In the editor: "It's heard".
- Timers can say something else after the first time: `say_again`. In the
  editor: "After the first time, it says". Left empty, `say` plays every
  time.
- Timers can go off at random: `after: 3-8` or `every: 5-10` picks a
  random number of turns in the range, again each time for `every`. The
  same seed plays the same way, so walkthroughs still replay. In the
  editor, type a range in the timer's turns.
- Feeling your way in the dark: things the player has seen (or put down,
  or felt for) can still be reached in the dark, so TAKE KEY and LIGHT THE
  CANDLE WITH THE LIGHTER work. FEEL AROUND finds what's within reach,
  FEEL FOR THE KEY finds it and picks it up, and LOOK in the dark says
  what the player knows is there. `findable_in_dark: true` (in the
  editor: "Can be found by feel in the dark") for a light switch or
  matches the player can always find.
- The editor's **Puzzles** tab: one way through the game, step by step
  (find, unlock, reach, end), and what can never happen: rooms nobody can
  reach, keys that open nothing, locks with no key, hidden things nothing
  reveals, endings nothing leads to.
- Test play keeps what the game didn't understand, under **Not
  understood**, each with a fix: add the word as another name for
  something in that room, make an unknown verb mean one the game knows,
  or add the word as an object.
- Renaming in the editor reaches everything: changing a room's, object's
  or character's ID updates every reference (exits, doors, `via`, keys,
  reveal and timer rules, characters' things) and the Lua code that names
  it (`world.hall`), in game.yaml and in script files, and the room keeps
  its place on the map. Renaming an ending, conversation, timer, goal or
  verb updates `end_game("…")`, `open_choice("…")`, `start_timer("…")`,
  `start_goal("…")` and a verb's handlers. One UNDO takes it all back.
- Pictures: a room, object or character can have a `picture` (a file
  beside the game, or an https address) and `alt` words for players who
  can't see it. A room's shows above its description and with the map; a
  thing's when it's examined, for clues the player has to see. In the
  editor: a **Picture** section on each, and **Add component → Picture**.
  The Small House starter has a note on the fridge to look at.
- Players can talk more naturally:
  - Lists with commas (TAKE THE KEY, THE COIN AND THE LAMP), MY HAT, THE
    BRASS ONE, THE OTHER ONE, THE SECOND KEY, TWO COINS. Identical things
    (two gold coins) no longer get "which do you mean?".
  - Extra words are fine: CAN I GO EAST, TRY WEST, LET'S GO SOUTH, LOOK
    CLOSELY AT IT, GO EAST PLEASE, SAVE GAME, TAKE IT AGAIN. UNDO UNDO
    undoes twice.
  - Questions: WHAT'S HERE, WHAT AM I CARRYING, CHECK INVENTORY, WHO IS
    HERE, WHAT CAN I DO (suggestions for right now), ABOUT (title, author,
    version).
  - The floor: TAKE THE KEY FROM THE FLOOR, PUT IT ON THE GROUND. X ALL
    looks at everything.
  - LOOK UP / LOOK DOWN where there's no way says "You see nothing special
    that way."
  - A place's name goes there: KITCHEN, GO KITCHEN, next door or anywhere
    visited.
  - HELLO and HELLO KEEPER start a conversation; SAY XYZZY is a verb games
    can hook.
  - Short replies, which games can hook or reword, for JUMP, SIT, LIE
    DOWN, SLEEP, SING, DANCE, PRAY, CRY, LAUGH, YES, NO, THANKS, SORRY,
    XYZZY, and CUT, TIE, DIG, KNOCK, WAVE, POINT.
  - DRINK (new trait `drinkable`), CLIMB, WAKE, and SHOW … TO ….
- Exits can say what they go by: `via: stairs` makes CLIMB THE STAIRS, GO
  DOWN THE STAIRS and CROSS THE BRIDGE work, and the staircase is there on
  both sides. In the editor: **Goes by** on a way out.
- `aliases:`: a game's own words for words it knows (`using: with`, `port:
  west`), anywhere in a command. In the editor: **Words** in the game's
  details.
- Several commands now mean something: DANCE, SING, JUMP and the other
  words above used to be "I don't know the verb". A game's `parse_failed`
  handler no longer sees them; hook the verb instead (`on: { dance: ... }`).
- More of what players type works:
  - AND joins commands: TAKE THE KEY AND OPEN THE DOOR. It still joins
    things, too: TAKE THE KEY AND THE LAMP, and now TAKE ALL BUT THE KEY.
  - USE: USE LAMP lights it, USE HAT wears it, USE KEY ON DOOR unlocks
    it, USE SWORD ON GOBLIN attacks. The game's own handlers for those
    verbs still run. A game that defines its own `use` verb now gets a
    warning, as with any standard verb.
  - GO TO THE KITCHEN walks to a place visited before, a turn a step,
    stopping if anything happens. GO BACK returns to the last room. UNDO
    takes back the whole walk.
  - WAIT 3, WAIT TEN MINUTES, WAIT UNTIL DUSK, WAIT UNTIL 9:30 PM wait,
    stopping as soon as anything happens.
  - X ROOM, SEARCH HERE and LOOK AT THE HALL work on the room; WHERE AM I
    is LOOK; EXITS lists the ways out.
  - Fixed: EXITS no longer means EXIT (go out), SING no longer means S
    (south), and BRIEF is no longer taken as a misspelling of BRIBE.
- Doors can be named by the way they lead: UNLOCK NW DOOR, OPEN THE EAST
  DOOR. When two doors have the same name, "Which do you mean?" says
  which way each one goes, and NORTHWEST is enough as a reply.
- ASK and TELL start a conversation, like TALK TO: ASK KEEPER, ASK
  KEEPER ABOUT THE GATE, TELL KEEPER ABOUT THE STORM. With only one person
  here to talk to, TALK or ASK ABOUT THE GATE needs no name. (ASK … TO is
  still an order.)
- Read aloud, in *Text and colours* while playing: new text is spoken
  with your browser's own voices, at normal speed, slower or faster.
  Escape stops it. Off unless you turn it on.
- A *Very large* text size.
- For screen readers: the map is described in words (where you are, the
  places you've been and their ways out), the story is marked with the
  game's language, and the home page's demo has a proper label.
- Goals: a `goals:` section, each with text, and conditions for when it's
  added and when it's done. JOURNAL lists them, and a notice says when one
  is added or done. In the editor: Add component → Goal.
- The time of day: a `clock:` section starts a clock that moves on with
  each turn, shown on the status line. TIME tells the player; text and
  scripts can check `clock.night`, `clock.hour` and more; `pass_time` and
  `set_clock` move it on; and timers can go off `at: dusk` or at a time.
  In the editor: Add component → Time of day.
- The Small House starter template now has goals, the time of day, and
  a clock that strikes nine.
- Hints: a `hints:` section lists puzzles in the order players meet them,
  each with steps from a nudge to the answer and a `when` that's true while
  it's unsolved. HINT shows the next step for the puzzle the player is on
  (it used to be the same as HELP). The starter templates have hints, and
  the editor can add and edit them (Add component → Hint).
- Record a walkthrough in the editor: play from the start in Test play,
  then **Save as walkthrough**. After every change, the editor replays it,
  says in Test play whether it still wins, and warns under Problems when it
  doesn't. Starter templates and copied examples bring theirs along. Test
  play now always uses the same random numbers, so recordings replay the
  same way.
- Download a game as one web page: the editor's **Download** offers a
  single .html file with the game and engine inside, that plays anywhere,
  even offline. The zip of the game's files is still there too.

- A Parchment look in *Text and colours*: ink on old paper, room names in
  small capitals, and a drop capital at the start of each room's
  description. Games can choose it too, with
  `presentation: { theme: parchment }`.
- *Text appears: letter by letter*, in *Text and colours* while playing:
  new text is typed out like a typewriter. It's off unless you turn it on,
  and any key, click or tap shows the rest at once.

- Starter templates in the editor: start a new game from "A Small House"
  (five rooms, a locked front door and keys to find), "The Locked Door"
  (a dungeon cell, a sleeping guard and an iron door) or "Market Day"
  (someone to talk to, a baker, and someone to help), instead of a blank
  room. Each is a small, finished game to change into your own.

- The editor's "+ Room" button is now "Add component", for anything a game
  can have: a room (added straight away, as before), an object or a
  character (it asks which room first), a door, a side, an ending, a
  conversation, a verb, a timer, a variable or a script file.

- Room colours in the editor, to group rooms into areas (the castle, the
  forest): pick one for each room, and it shows on the player's map too.
  They're kept in `map.yaml`, so the game itself doesn't change.
- An "Editor theme" choice on the editor's map: the site's colours, or
  Blueprint (white lines on blue), so working on the map feels different
  from playing. It's remembered in your browser.

- Room descriptions end with "Known exits:", and list only the ways out
  that are open to the player right now: not an exit whose `when` is
  false, and not a secret door that hasn't been found. Games can turn the
  list off with `settings: { list_exits: false }`, or per room; the
  editor has a switch for both.

- Test play in the editor: play your game in the editor's bottom panel
  while you work on it, from the start or from the room selected on the
  map, with the room you're in lit up on the map. It tells you when you've
  changed the game since starting.

- When a game ends on the website, you can still type: UNDO, RESTORE and
  RESTART work, as the game says.

- For AI assistants: a guide to writing games with one (format, common
  mistakes, a complete checked example), every docs page as Markdown next
  to its HTML, and `llms.txt`, `llms-full.txt` and `sitemap.xml` at the
  site's root.

- Send feedback from anywhere on the site: report something broken or
  confusing, or an idea for playing, the editor or the website. It says
  which page (or which game, room and turn) you were on and which version
  of the site you're seeing, and shows everything it sends first.

- Download a transcript of your game, as a text file, from Saves or when
  the game ends.
- Endings found: a game's card on the home page and in the gallery says
  "You've found 2 of 3 endings", and so does the end of a game. It's kept
  in your browser.
- Games can choose their look on the website: `presentation: { theme:
  terminal, accent: "#c33" }`, or "How it looks" in the editor. Players
  see it unless they've picked their own colours.
- In the editor, drag a selected room's corner to make its box bigger or
  smaller, so a great hall looks bigger than a closet. It's only for
  picturing the map (it's kept in `map.yaml`), and the player's map draws
  rooms the same size.
- Barter: TRADE RING FOR LAMP swaps something you carry for something a
  merchant sells, no money needed, if it's worth enough to them. See
  [Trading](README.md#trading).
- Pushing and climbing: `pushable` things are too heavy to carry but can
  be pushed into the next room (PUSH CRATE NORTH), and `high: true`
  things are out of reach until the player stands on something (CLIMB ON
  THE CRATE). See [Pushing and climbing](README.md#pushing-and-climbing).
  CLIMB UP and CLIMB DOWN now go up and down.

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
