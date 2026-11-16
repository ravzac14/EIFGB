# EIFGB

EIFGB turns a game description into a playable interactive fiction game.
You describe rooms, objects, characters and puzzles in a YAML file, add
Lua where you need custom behaviour, and EIFGB handles the rest: a
forgiving command parser, saving, undo, menus and dialogue, and endings.

```
> look under the mat
You lift the corner of the mat. Underneath is a small key, which you take.

> unlock door
You unlock the pantry door with the small key.
```

> **Status: early.** Everything runs from the command line for now. A web
> player and an online editor are planned. The file format may still change
> before 1.0.

- [Quick start](#quick-start)
- [Tutorial: Midnight Snack](#tutorial-midnight-snack)
- [States and descriptions](#states-and-descriptions)
- [Command reference](#command-reference)
- [Cheat sheet](#cheat-sheet)
- [Where to go next](#where-to-go-next)
- [For contributors](#for-contributors)

## Quick start

You need [Rust](https://rustup.rs) and a C compiler. The C compiler builds
the bundled Lua; on Ubuntu or WSL, install it with `sudo apt install build-essential`.

```
git clone <this repository>
cd EIFGB
cargo build
```

That creates the `eifgb` tool at `./target/debug/eifgb`. Try the sample game:

```
./target/debug/eifgb play spec/examples/lighthouse/game.yaml
```

Type commands like `look`, `search pool`, `go north`, `talk to keeper`, and
`undo`. `help` lists the basics and `quit` exits.

## Tutorial: Midnight Snack

In this tutorial you build a small game in five steps: a hungry player, a
locked pantry, a key hidden under a mat, and an apple. The finished game
is in [`spec/examples/midnight_snack/`](spec/examples/midnight_snack/game.yaml).

Every command below is run from the repository folder.

### Step 1: one room

Create `games/midnight_snack/game.yaml`:

```yaml
format: eifgb/0.1

game:
  id: midnight_snack
  title: "Midnight Snack"
  version: "1.0"
  start: kitchen
  intro: You wake up hungry. Something in the pantry is calling your name.

rooms:
  kitchen:
    name: Kitchen
    description: A small kitchen, lit only by the glow of the fridge.
```

Every game needs `format`, a `game` section and at least one room. `start`
says which room the player begins in. IDs like `kitchen` are how you refer
to things elsewhere in the file. They use lowercase letters, digits and `_`.

Check it, then play it:

```
./target/debug/eifgb check games/midnight_snack/game.yaml
./target/debug/eifgb play games/midnight_snack/game.yaml
```

```
MIDNIGHT SNACK

You wake up hungry. Something in the pantry is calling your name.

Kitchen
A small kitchen, lit only by the glow of the fridge.
There's no obvious way out.
```

Type `quit` to leave.

### Step 2: a second room

Add an exit from the kitchen and a pantry to go to:

```yaml
rooms:
  kitchen:
    name: Kitchen
    description: A small kitchen, lit only by the glow of the fridge.
    exits:
      north: pantry

  pantry:
    name: Pantry
    description: Shelves of jars, and the sweet smell of apples.
    exits:
      south: kitchen
```

Exits are one-way, so the pantry needs its own exit back. Players can type
`north`, `n`, `go north` or `walk north`.

### Step 3: an apple and an ending

Add an `objects` section and an `endings` section at the end of the file:

```yaml
objects:
  apple:
    name: red apple
    synonyms: [fruit]
    in: pantry
    traits: [portable, edible]
    description: Shiny, red and perfect.
    on:
      after_eat: end_game("fed")

endings:
  fed:
    kind: win
    text: Crisp, sweet and worth getting up for. Time to go back to bed.
```

- **`name`** is what the game calls the object. Players can use any of its
  words: `apple`, `red apple`, and also `fruit`, because it's listed in `synonyms`.
- **`in`** says where the object starts. That can be a room, a container,
  or `player` to start in the player's hands.
- **`traits`** switch on built-in behaviour: `portable` means it can be
  taken, and `edible` means it can be eaten.
- **`on`** holds event handlers written in Lua. `after_eat` runs after the
  built-in EAT succeeds, and `end_game("fed")` ends the game with the `fed` ending.

Play it: `n`, `take fruit`, `eat it`.

```
> eat it
You eat the red apple.

Crisp, sweet and worth getting up for. Time to go back to bed.

*** You have won ***
```

### Step 4: a locked door

A puzzle needs an obstacle. Route the kitchen's north exit through a
door, then add the door and a key to `objects`:

```yaml
rooms:
  kitchen:
    name: Kitchen
    description: A small kitchen, lit only by the glow of the fridge.
    exits:
      north:
        to: pantry
        through: pantry_door
```

```yaml
objects:
  pantry_door:
    name: pantry door
    synonyms: [door]
    in: kitchen
    traits: [door, openable, lockable, scenery]
    state: { locked: true }
    unlocks_with: small_key
    description: A sturdy wooden door.

  small_key:
    name: small key
    synonyms: [key]
    in: kitchen
    traits: [portable]
    description: A small brass key.

  # ...the apple stays as before
```

- **`state`** holds values that change during play. `locked` and `open` are
  ones the built-in verbs understand.
- **`unlocks_with`** makes `unlock door` work without any script. If the
  player is carrying the key, it's used automatically.
- **`scenery`** stops the door from being listed among the room's contents.
  You've already mentioned it in the description.
- You don't need to describe whether the door is open, closed or locked.
  The game adds that itself: `x door` gives "A sturdy wooden door. It's
  locked.", and the kitchen mentions "The pantry door is locked." Things
  that are listed get a tag instead, like "a wooden box (open)". The same
  goes for states you invent: give the apple `state: { ripe: true }` and
  `x apple` adds "It's ripe." [States and descriptions](#states-and-descriptions)
  has the details.

`check` catches mistakes before you play. Misspell the key as `smal_key` and you get:

```
error: objects.pantry_door.unlocks_with: unknown ID 'smal_key' (did you mean 'small_key'?)
1 error(s), 0 warning(s)
```

Fix it and play: `n` (locked), `take key`, `unlock door`, `n`. An unlocked
door opens on its own as you walk through it.

Doors work from both sides. The pantry's `south: kitchen` exit now goes
through the pantry door too, without you changing it, and the door can be
examined, closed and locked from inside the pantry. If a room has no exit
back at all, one is added in the opposite direction.

If you'd rather describe states yourself, turn the automatic descriptions
off with a top-level `settings` section:

```yaml
settings:
  describe_state: false
```

### Step 5: hide the key

The key lying in plain sight is too easy. Hide it under a mat. Give the key
the state `hidden: true`, which keeps it out of the game, and say what
reveals it with `revealed_by`:

```yaml
  small_key:
    name: small key
    synonyms: [key]
    in: kitchen
    traits: [portable]
    state: { hidden: true }
    revealed_by: search mat
    description: A small brass key.
```

Then add the mat:

```yaml
  mat:
    name: worn mat
    synonyms: [rug]
    in: kitchen
    traits: [scenery]
    description: A threadbare mat.${if world.small_key.hidden} One corner looks lumpy.${end}
```

- **`revealed_by: search mat`** means searching the mat reveals the key.
  Any way of saying it counts: `search mat`, `look under the mat`, `look
  under rug`. The game replies "You find a small key." and the key appears
  where it is. A list such as `[search mat, examine mat]` lets any of
  several actions reveal it.
- **`${if ...}...${else}...${end}`** makes text react to the game. Here it
  gives players a clue, and the lumpy corner disappears once the key is
  found. Inside a template, `world.<id>` is any object and `self` is the
  object the text belongs to. If an object's own description already talks
  about one of its states this way, the automatic sentence about that state
  is left out, so nothing is said twice.

For anything `revealed_by` can't express, such as revealing only on the
third search or handing the key straight to the player, write a Lua handler
instead. See [`spec/examples/lighthouse/game.yaml`](spec/examples/lighthouse/game.yaml),
where searching the tide pool finds a key and scores points.

Finally, mention the mat in the kitchen description so players know it's there:

```yaml
    description: |
      A small kitchen, lit only by the glow of the fridge. A worn mat
      lies in front of the pantry door to the north.
```

A multi-line description is reflowed when displayed, so wrap the lines in
your file however you like. A blank line starts a new paragraph.

### Step 6: a walkthrough file

Save a winning playthrough as `games/midnight_snack/walkthrough.txt`, one
command per line. Lines starting with `#` are comments:

```
n
x mat
look under the mat
take key
unlock door
n
take the fruit
eat it
```

Replay it at any time, to check that your changes haven't broken the game:

```
./target/debug/eifgb play games/midnight_snack/game.yaml --commands games/midnight_snack/walkthrough.txt
```

That's a complete game. Compare yours with
[`spec/examples/midnight_snack/game.yaml`](spec/examples/midnight_snack/game.yaml).

## States and descriptions

Objects change during play: doors open, lamps get lit, oil runs out. EIFGB
keeps track of these changes as **state**, and describes them for you, so
most descriptions never need to mention them.

### Declaring state

List an object's starting state under `state:`. Values can be true/false,
numbers or text:

```yaml
objects:
  lantern:
    name: brass lantern
    synonyms: [lantern, lamp]
    in: shed
    traits: [portable]
    state: { lit: false, oil: 3, _times_lit: 0 }
    description: A dented brass lantern.
    on:
      light: |
        self.lit = true
        self.oil = self.oil - 1
        self._times_lit = self._times_lit + 1
        say("The wick catches.")

  cupboard:
    name: cupboard
    in: shed
    traits: [container, openable, scenery]

verbs:
  light:
    patterns: ["light {target}"]
    default: "That won't light."
```

Scripts read and change state through the object (`self.lit = true`, or
`world.lantern.oil` from anywhere). They can also add fields that weren't
declared. The built-in verbs keep a few states up to date for you:

| State | Set by | Meaning |
|---|---|---|
| `open` | OPEN and CLOSE, on `openable` things | Open or shut. A container that isn't `openable` is always open. |
| `locked` | LOCK and UNLOCK, on `lockable` things | Locked. A locked thing can't be opened. |
| `worn` | WEAR and TAKE OFF, on `wearable` things | Being worn. |
| `hidden` | `revealed_by`, or scripts | Out of the game: it can't be seen or referred to until revealed. A hidden door is a secret door: its exits aren't listed or usable until it's found. |
| `blocked` | scripts, on `door`s | Held shut: barred, bolted or wedged. `true` means from both sides; a room ID (`blocked: cellar`) or a list of room IDs means only from those sides. A blocked door is always closed. Opening it from an unblocked side lifts the block. |

### What players see

With nothing more than the declarations above, playing gives:

```
Garden Shed
A cramped wooden shed. The cupboard is closed.
You can see a brass lantern here.
There's no obvious way out.

> x lantern
A dented brass lantern. It isn't lit. Its oil is 3.

> light lantern
The wick catches.

> x lantern
A dented brass lantern. It's lit. Its oil is 2.

> take lantern
Taken.

> i
You're carrying a brass lantern (lit).
```

State is mentioned in three places:

| Where | What's added | Example |
|---|---|---|
| Examining a thing | Every state, after its description | "It isn't lit. Its oil is 3." |
| Lists (room contents, inventory, inside containers) | Tags for states that are "on" | "a brass lantern (lit)", "an iron gate (locked)" |
| Room descriptions | A sentence for things that aren't listed (scenery), for "on" states | "The cupboard is closed." |

Lists and rooms leave out "off" states and numbers, because "a lantern
(not lit, oil: 3)" in every list would be clutter. Examine the thing to
see everything.

### How your own states are worded

The sentence comes from the field's name and value:

| Field | Examining | In lists |
|---|---|---|
| `lit: true` | "It's lit." | "(lit)" |
| `lit: false` | "It isn't lit." | *(nothing)* |
| `is_broken: true` | "It's broken." (a leading `is_` is dropped) | "(broken)" |
| `oil: 3` | "Its oil is 3." | *(nothing)* |
| `color: red` | "Its color is red." | *(nothing)* |
| `_times_lit: 1` | *(private: never shown)* | *(nothing)* |

Some tips:
- **Name true/false states with an adjective:** `lit`, `broken`, `ripe`, `fueled`, `full`. Underscores become spaces, so `half_full` reads "It's half full."
- **Start bookkeeping fields with `_`**, like `_times_lit` or `_visits`. They work in scripts like any other field but are never described.
- **Lists and tables in state** are never described.

### Writing your own state text

When you want better prose than the automatic sentence, write it into the
description with a template. Any field your description reads is left out
of the automatic sentences, so it isn't said twice:

```yaml
    description: "A dented brass lantern${if self.lit}, glowing warmly${else}, cold and dark${end}."
```

```
> x lantern
A dented brass lantern, cold and dark. Its oil is 3.
```

`lit` is now covered by your text, and `oil` is still described
automatically. For a door, mentioning either `open` or `locked` covers
both. A room description that mentions `world.cupboard.open` stops the room
from adding "The cupboard is closed."

### Rewording or turning it off

Every sentence comes from a message you can override under `messages:`.
For example:

```yaml
messages:
  state.custom.value: "It has ${args.value} ${args.state} left."
  state.open: "It's standing open."
```

```
> x lantern
A dented brass lantern. It isn't lit. It has 3 oil left.

> x cupboard
It's standing open.
```

| Message | Used for | Default |
|---|---|---|
| `state.open`, `state.closed`, `state.locked`, `state.worn` | Examining | "It's open." … "You're wearing it." |
| `state.tag.open`, `.closed`, `.locked`, `.worn` | Lists | "open" … "worn" |
| `state.here.open`, `.closed`, `.locked` | Room descriptions | "${The(args.target)} is open." … |
| `state.custom.on`, `state.custom.off` | Examining your true/false fields | "It's ${args.state}." / "It isn't ${args.state}." |
| `state.custom.value` | Examining your number and text fields | "Its ${args.state} is ${args.value}." |
| `state.here.custom.on` | Room descriptions, your fields | "${The(args.target)} is ${args.state}." |

To turn automatic state descriptions off for the whole game and write all
state text yourself:

```yaml
settings:
  describe_state: false
```

```
Garden Shed
A cramped wooden shed.
You can see a brass lantern here.
There's no obvious way out.

> x lantern
A dented brass lantern.
```

## Command reference

| Command | What it does |
|---|---|
| `eifgb check <game.yaml>` | Validates the game and lists errors and warnings. Exits non-zero on errors. |
| `eifgb play <game.yaml>` | Plays the game in the terminal. |
| `eifgb build <game.yaml> [-o out.json] [--pretty]` | Compiles the game to a story file (JSON), the format players will load. |

Options for `play`:

| Option | Meaning |
|---|---|
| `--commands <file>` | Run commands from a file instead of the keyboard, echoing each one. |
| `--seed <n>` | Fix the random seed. The same seed and commands always give the same game. |
| `--save-file <path>` | Where SAVE and RESTORE keep the saved game. Defaults to `<game id>.sav`. |

In-game commands that every game understands include LOOK, EXAMINE (X),
TAKE, DROP, PUT … IN/ON, OPEN, CLOSE, LOCK, UNLOCK, SEARCH, TALK TO, WEAR,
TAKE OFF, EAT, GO THROUGH / ENTER (a door), INVENTORY (I), WAIT (Z), directions (N, S, E, W, NE, NW, SE,
SW, UP, DOWN, IN, OUT), plus UNDO, AGAIN (G), SAVE, RESTORE, RESTART,
SCORE, HELP and QUIT.

## Cheat sheet

**Top-level sections:** `format`, `game`, `settings`, `vars`, `player`, `rooms`,
`objects`, `characters`, `verbs`, `choices`, `on`, `endings`, `messages`,
`scripts`.

**Traits:** `portable`, `scenery`, `container`, `supporter`, `openable`,
`lockable`, `door`, `wearable`, `edible`, `light_source`. `open` is
shorthand for `state: { open: true }`.

**Settings:** `describe_state` (default `true`) describes state
automatically in examine text, room descriptions and lists, including
your own fields. See [States and descriptions](#states-and-descriptions).

**Hidden things:** `state: { hidden: true }` plus `revealed_by: search mat`
(or a list) reveals it when the player does that. SEARCH and custom verbs
with no handler say "You find a small key." instead of their usual reply.
Other actions add it after: "You open the desk. You find a note." Hide a
door to make a secret door.

**Doors:** give the door the `door` trait (usually with `openable` and
`lockable`), put it in one of the rooms with `in:`, and send an exit
`through:` it. It works from both rooms. The return exit is linked or
added for you. To hold a door shut (barred, bolted, wedged), use the
`blocked` state:

```yaml
  trapdoor:
    name: trapdoor
    in: hall
    traits: [door, openable]
    state: { blocked: cellar }    # bolted on the hall side: won't open from the cellar
```

From the cellar, OPEN TRAPDOOR and UP both get "The trapdoor won't open
from this side.", and examining it says "It's closed. It's blocked from
this side." From the hall it opens normally, which slides the bolt back.
Scripts change it during play: `world.trapdoor.blocked = "cellar"` (one
side), `= true` (both sides: "The trapdoor won't open."), or `= false`.
Blocking an open door closes it.

**Exits:** `north: pantry` (to a room); `south: "The sea. Not tonight."`
(always blocked, with this message); or a map with `to`, `through` (a
door), `when` (a Lua condition) and `blocked` (the message when `when` is false).
A direction can also take a list:

```yaml
exits:
  north:                       # conditional route: the first that applies wins
    - { to: bridge, when: vars.bridge_down }
    - { blocked: "The bridge is raised." }
  east:                        # separate passages: GO EAST asks which door
    - { to: library, through: red_door }
    - { to: study, through: blue_door }
```

Players can also say GO THROUGH, WALK THROUGH or ENTER a door.

**Handlers** go under `on:`. The name is a verb, optionally with
`before_` or `after_` in front:

| Handler | Runs |
|---|---|
| `before_take` | Before the built-in behaviour. `return block("message")` stops the command. |
| `take` | Instead of the built-in behaviour. |
| `after_take` | After the built-in behaviour succeeds. |

Handlers on the object run first, then the room's, then the global ones
(the top-level `on:`). Rooms also have `enter`, `leave` and `turn` events.
The top level has `start`, `turn` and `parse_failed`.

**Text:** `${expr}` inserts a value; `${if cond}…${elseif cond}…${else}…${end}`
chooses text. `the(x)`, `The(x)`, `a(x)` and `A(x)` add articles.

**Lua:** `world.<id>`, `player`, `here`, `self`, `args.target`, `args.tool`,
`vars.<name>`, `game.turn`, `game.score`; `say()`, `block()`, `score(n)`,
`end_game(id)`, `open_choice(id)`, `random(a, b)`; on objects,
`:move_to(place)`, `:is_in(holder)`, `:contents()`, `:has(trait)`.

**Custom verbs:**

```yaml
verbs:
  light:
    patterns: ["light {target}", "ignite {target}"]
    default: "That won't burn."
```

`{target}` is the thing acted on and `{tool}` is a second thing (`fill {target} with {tool}`).

## Where to go next

- **[`spec/examples/lighthouse/game.yaml`](spec/examples/lighthouse/game.yaml)**
  is a fuller game with comments. It has custom verbs, a dialogue with a
  character, a menu puzzle, global variables, a turn limit, a scoring
  system and a Lua helper file.
- **[`spec/FORMAT.md`](spec/FORMAT.md)** is the full reference: every
  section and field, the scripting API and sandbox rules, and the
  **Parser behavior** section, which shows how the game answers typos,
  ambiguous commands and things it doesn't understand.

## For contributors

```
crates/
  eifgb-ir/        compiled story format (what every player runtime reads)
  eifgb-stdlib/    versioned standard library: verbs, synonyms, messages
  eifgb-compiler/  YAML + Lua source → validated story
  eifgb-engine/    world model, parser, turn loop (no I/O; also targets the web)
  eifgb-lua/       Lua 5.4 script host (sandboxed)
  eifgb-cli/       the `eifgb` command
spec/
  FORMAT.md        format reference and design notes
  examples/        example games; each walkthrough.txt is played by the tests
```

Run the tests with `cargo test --workspace` and lint with
`cargo clippy --workspace --all-targets`. Every example that has a
`walkthrough.txt` is played to a win by the test suite, so the tutorial
above stays in sync with the engine.
