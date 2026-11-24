# Writing games with an AI assistant

A compact guide for language models and coding agents that write games
for the Interactive Fiction Game Builder/Player (EIFGB), and for people
pasting it into one. It covers the file format, the rules that most often
trip up a first draft, and how to check and play a game. The full guide
and the [format reference](../spec/FORMAT.md) go deeper.

> [!TIP]
> Every page of these docs is also published as Markdown, next to the
> HTML (`docs/index.md`, `docs/format.md`, `docs/ai.md` and so on). The
> site's [`llms.txt`](../web/llms.txt) lists them all, and
> [`llms-full.txt`](../web/llms-full.txt) has them in one file.

## Working in a chat

Most authors talk to you in a chat, with no way for you to run anything.
Then:

- **Reply with the whole game in one fenced block**, ```` ```yaml ````, from
  `format:` to the end, every time. Not a part, not a diff: the author
  pastes the whole block into the editor (*Make a game with your AI*, on
  its first page), and it replaces what was there.
- **Keep it one file.** Write Lua inline in handlers (`on: { take: … }`)
  rather than in separate script files.
- **Add a walkthrough** in a second fenced block, ```` ```text ````: the
  commands that win, one a line. The editor plays it after every change.
- **When the author pastes back problems** (from the editor's
  *Copy for your AI* button), each has a path in the file and usually a
  fix. Fix every one, and send the whole file again.

## The loop

1. Write `game.yaml` (and any `.lua` files it lists under `scripts:`).
2. Check it: `eifgb check game.yaml`. Every mistake is reported with its
   path in the file (`objects.cat.talk: …`) and usually a fix ("did you
   mean …", "move this to 'characters'"). Fix them all and check again.
   The editor on the website shows the same problems as you type.
3. Play it: `eifgb play game.yaml --commands walkthrough.txt` plays a file
   of commands, one per line, and prints every reply. A walkthrough that
   reaches an ending is the best test that a game can be finished.
4. Share it: a `game.yaml` hosted anywhere public plays at
   `https://ravzac14.github.io/EIFGB/play.html?game=<its address>`.

## The shape of a game

```yaml
format: eifgb/0.1          # required, always this
game:                      # required: id, title, version, start
  id: my_game              # lowercase letters, digits and _
  title: "My Game"
  version: "1.0"           # quoted, so "1.10" stays 1.10
  start: hall              # the room the player starts in
  intro: Shown once, before the first room.
  max_score: 10            # optional
rooms: {}                  # where things are
objects: {}                # things: keys, doors, boxes, furniture
characters: {}             # anything with behaviour, conversation or health
endings: {}                # every ending end_game() can name
```

Other top-level sections: `settings`, `presentation`, `vars`, `player`,
`verbs`, `choices`, `on`, `timers`, `factions`, `goals`, `scenes`, `hints`,
`clock`, `weather`, `aliases`, `messages`, `scripts`. An unknown section or
field is an error, not ignored, so a typo is caught.

## Rules that matter

- **IDs** (the keys under `rooms:`, `objects:`…) are lowercase
  identifiers, unique across the whole game. They're what `in:`, `start:`,
  exits and scripts refer to. Players never see them: they use `name` and
  `synonyms`.
- **Placing things:** `in:` a room, another object (a container or
  supporter), a character, or `player` (carried at the start).
- **Objects vs characters:** anything with `behavior`, `talk`, `health`,
  `faction` or the like goes under `characters:`, not `objects:`.
- **Traits** say what a thing can do: `portable` (can be taken), `scenery`
  (described by the room, not listed), `container`, `supporter`,
  `openable`, `lockable`, `door`, `wearable`, `edible`, `drinkable`, `light_source`,
  `enterable`, `transparent`, `flammable`, `destructible`, `pushable`.
  Things without `portable` can't be picked up.
- **State** is a map of values: `state: { locked: true, lit: false }`.
  Built-in fields: `open`, `locked`, `hidden`, `worn`, `lit`. Descriptions
  of open, closed, locked and so on are added automatically: don't write
  them into the description.
- **Exits:** `north: pantry`; `south: "The sea. Not tonight."` (always
  blocked, with that message); or `{ to, through, when, blocked, via }`.
  `via: stairs` names a thing the way goes by, so CLIMB THE STAIRS works.
  Directions: north, south, east, west, northeast, northwest, southeast,
  southwest, up, down, in, out. Room descriptions end with "Known exits:"
  and the ways out open right now; `settings: { list_exits: false }` (or
  `list_exits: false` on a room) leaves that out.
- **Doors:** an object with the `door` trait (usually `openable`, maybe
  `lockable` with `unlocks_with: <key id>`), placed `in:` one of the two
  rooms, and an exit `through:` it. The way back is added automatically.
- **Pictures:** `picture: { file: pictures/hall.jpg, alt: "What it shows" }`
  on a room, object or character. Always write `alt`, with any clue in it.
- **Behaviour without scripts:** `needs_flame`, `fuel`, `burns_away` on
  light sources; `holds_liquid`/`full_of` and `liquid: water` for liquids;
  `uses`/`breaks`; `code` on lockable things; `text` for READ; `sound` and
  `smell`; `outdoors` rooms with a top-level `weather:`. Timers take
  `chance: 10%`.
- **Aliases:** a top-level `aliases:` map gives the game's own words for
  words it knows (`using: with`, `port: west`). Don't add verbs this way.
- **Hidden things:** `state: { hidden: true }` with `revealed_by: search
  mat` (a verb and a thing's ID).
- **Dark rooms:** `dark: true`. The player sees nothing, and can only
  handle what they carry, until a `light_source` that's `lit` is there.
- **Up high:** `high: true` puts a thing (and what's on it) out of reach
  until the player stands on a supporter that's `enterable`.
- **Endings:** `end_game("id")` needs that ID under `endings:`, each with
  `kind` (`win`, `loss` or `neutral`) and `text`.
- **Handlers** go under `on:` on a thing, room or the top level, named by
  verb: `before_take` (return `block("message")` to stop it), `take`
  (instead of the built-in), `after_take`. Rooms also have `enter`,
  `leave` and `turn`; the top level has `start`, `turn` and `parse_failed`.
- **Lua in handlers:** `world.<id>`, `player`, `here`, `self`, `vars.<name>`,
  `args.target`, `args.tool`; `say()`, `block()`, `score(n)`,
  `end_game(id)`, `open_choice(id)`, `random(a, b)`, `start_timer(id)`;
  on things `:move_to(place)`, `:is_in(holder)`, `:contents()`,
  `:has(trait)`, and state as fields (`self.lit`, `world.door.open = true`).
- **Text templates:** `${expr}` inserts a value, and
  `${if cond}…${else}…${end}` chooses text. `the(x)`, `The(x)`, `a(x)`
  and `A(x)` add articles.
- **Text that varies:** a description, a verb's `default`, a timer's
  `say`, a conversation's `text` or `say`, or a message can be a list
  (each in turn, then the last for ever), `{ first: …, later: … }`,
  `{ cycle: [...] }` or `{ random: [...] }`. A room's visits are
  `here.visits` (1 on the first).
- **Your own verbs:** under `verbs:`, with `patterns` such as
  `"flip {target}"` (and `{tool}` for a second thing), and a `default`
  reply. Handle them on things with `on: { flip: … }`.
- **Conversations:** a character's `talk: <choice id>` opens a choice
  graph under `choices:`: a `start` node, and `nodes` with `text` and
  `options`, each option with `text` and any of `say`, `do`, `goto`, `end`.
  Players answer by typing the option's number. In a conversation `self`
  is the one being talked to. An option's `when` hides it while false,
  unless it has `why_not: "reason"`, which shows it greyed with the reason;
  `disposition: 2` (or -2) changes how the one talked to feels.
- **Disposition:** how a character feels about the player, -10 to 10:
  `disposition: friendly` (or hostile, wary, neutral, devoted, or a
  number). Liked gifts, apologies and bribes warm them; attacks, theft and
  insults cool them. It shows on EXAMINE, in merchants' prices, and a
  hostile one won't talk or trade. Scripts read `self.disposition` and
  `self.feeling`.
- **More tricks:** `reflects: true` (a mirror: LOOK EAST WITH IT, ANGLE
  IT NORTH to light a dark room); `far: true` things need something that
  `reaches: true` (HOOK NEST WITH POLE); `splits_into: [lens, oil]` (TAKE
  LANTERN APART); characters with `answers_to: [captain]` come when the
  player CALLS OUT AS THE CAPTAIN; `friends: [baker]` hear how a
  character feels about the player; `remembers: { gift: ..., robbed: ...,
  attacked: ..., spared: ..., fooled: ... }` is said when they meet again.
  Throwing something distracts watchers; PLANT RUBY ON COOK frames them.
- **Moods:** `moods: { drunk: ... }` on rooms, things and characters
  replaces their description while the player's state `drunk` is true;
  a top-level `moods` section adds a line to every LOOK; `mood: { drunk:
  5 }` on something eaten or drunk brings one on.
- **Several player characters:** `playable: true` on characters, and
  `player: { plays: mara }` for who the player starts as; SWITCH TO TOBIN.
- **Tricks:** `combines: { with: hook, makes: grapple }` on an object
  (TIE ROPE TO HOOK makes a grappling hook that starts with no `in:`);
  `pushable` things barricade doors (PUSH WARDROBE AGAINST DOOR); objects
  with `marks: true` (chalk) let players mark rooms; `watches: [ruby]` on a
  character guards a thing, and `decoy_for: ruby` on a fake swaps for it
  unseen; PLAY DEAD fools anything that doesn't eat the dead.
- **Routines:** with a `clock:`, a character can keep to one: `routine: {
  "6:00": bakery, "13:00": square }`.
- **Scenes:** under `scenes:`, each with `title`, Lua `when` (begins) and
  `done` (ends), and `begins`/`ends` text. `scenes.storm` is `nil`,
  `"running"` or `"done"`; `start_scene(id)` and `end_scene(id)`.
- **Timers:** under `timers:`, `after: N` (or `every: N`) actions, where N
  can be a range for a random wait (`every: 5-10`), with
  `say`, `do` (Lua), `when` and `end` (an ending ID).
- **Goals:** under `goals:`, each with `text` for the JOURNAL, and Lua
  `when` (added once true; omit for from the start) and `done` (done once
  true). Notices tell the player as goals are added and done.
- **Time of day:** `clock: { start: "8:00", minutes_per_turn: 5 }`; text
  and scripts read `clock.time`, `clock.hour`, `clock.night`; timers can
  have `at: dusk` or `at: "19:00"`.
- **Hints:** under `hints:`, one entry per puzzle in the order players
  meet them, each with `steps` (a list of text, nudge first, answer last)
  and a Lua `when` that's true while the puzzle is unsolved. HINT shows the
  next step of the first one whose `when` is true. `not world.key.seen` is
  a good `when` for a thing to find: `seen` is true once the player has
  seen or held it.
- **Walkthroughs:** a line can say what the game should answer, after
  `=>`: `look under the mat => small key` (the reply must contain it).
  `walkthrough-<ending>.txt` is one that must reach that ending.

The standard commands (LOOK, TAKE, OPEN, PUT … IN, UNLOCK … WITH, TALK
TO, the directions, and many more) work in every game without any setup.
They're listed in [Commands every game understands](../README.md#commands-every-game-understands).

## A complete example

A small game that uses most of the above: a door, a container, something
up high, a dark room, a light, a character to talk to, a verb of its own,
a score and an ending. It compiles cleanly and the walkthrough below wins
it; the test suite checks both.

```yaml
format: eifgb/0.1

game:
  id: lantern_cellar
  title: "The Lantern Cellar"
  version: "1.0"
  start: cottage
  intro: The storm has knocked the power out. The fuse box is down in the cellar.
  max_score: 10

rooms:
  cottage:
    name: Cottage
    description: A low room with a cold stove and a high shelf. A trapdoor in the floor leads down.
    exits:
      down: { to: cellar, through: trapdoor }
      out: "The rain is coming down in sheets."

  cellar:
    name: Cellar
    dark: true
    description: Damp stone walls. A fuse box hangs by the stairs.
    exits:
      up: cottage

objects:
  trapdoor:
    name: trapdoor
    in: cottage
    traits: [door, openable, scenery]

  stove:
    name: stove
    in: cottage
    traits: [container, openable, scenery]
    description: A squat iron stove, long cold.

  matches:
    name: box of matches
    synonyms: [matches, box]
    in: stove
    traits: [portable]

  shelf:
    name: high shelf
    synonyms: [shelf]
    in: cottage
    high: true
    traits: [supporter, scenery]

  stool:
    name: wooden stool
    synonyms: [stool]
    in: cottage
    traits: [supporter, enterable]

  lantern:
    name: brass lantern
    synonyms: [lantern, lamp]
    in: shelf
    traits: [portable, light_source]
    state: { lit: false }
    description: Half full of oil.
    on:
      before_light: |
        if not world.matches:is_in(player) then
          return block("You'll need something to light it with.")
        end
      after_light: score(5)

  fuse_box:
    name: fuse box
    synonyms: [fuses, box, switch]
    in: cellar
    traits: [scenery]
    description: One switch has flipped down.
    on:
      flip: |
        score(5)
        end_game("power")

characters:
  cat:
    name: grey cat
    synonyms: [cat]
    in: cottage
    behavior: stationary
    talk: cat_talk
    description: The cat watches you with enormous eyes.

verbs:
  flip:
    patterns: ["flip {target}", "reset {target}", "flip up {target}"]
    default: "Nothing to flip there."

choices:
  cat_talk:
    start: hello
    nodes:
      hello:
        text: "The cat blinks at you, slowly."
        options:
          - text: "Ask where the lantern is"
            say: "The cat looks up at the high shelf."
            goto: hello
          - text: "Leave it be"
            end: true

endings:
  power:
    kind: win
    text: The lights flicker back on. The storm can do what it likes now.
```

Its walkthrough, `walkthrough.txt`:

```text
talk to cat
1
2
open stove
take matches
take lantern
stand on stool
take lantern
get off
light lantern
d
flip switch
```

## Common mistakes the checker reports

- A thing with `behavior` or `talk` under `objects:`: move it to
  `characters:`.
- `end_game("x")` with no `x` under `endings:`.
- An `in:`, `start:`, `unlocks_with:` or exit that names an ID that
  doesn't exist (with "did you mean" for near misses).
- A misspelled trait, field or section (unknown fields are errors).
- Lua syntax errors in any handler, reported with the line.

## More

- [The full guide](../README.md), with a step-by-step tutorial.
- [Scripting tutorial](SCRIPTING.md) and [advanced tutorial](ADVANCED.md):
  scripts, characters, fighting, factions.
- [Format reference](../spec/FORMAT.md): every field and message.
- [Example games](../spec/examples/lighthouse/game.yaml), with their files.
