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
  that are listed get a tag instead, like "a wooden box (open)".

`check` catches mistakes before you play. Misspell the key as `smal_key` and you get:

```
error: objects.pantry_door.unlocks_with: unknown ID 'smal_key' (did you mean 'small_key'?)
1 error(s), 0 warning(s)
```

Fix it and play: `n` (locked), `take key`, `unlock door`, `n`. An unlocked
door opens on its own as you walk through it.

If you'd rather describe states yourself, turn the automatic descriptions
off with a top-level `settings` section:

```yaml
settings:
  describe_state: false
```

### Step 5: hide the key

The key lying in plain sight is too easy. Hide it under a mat. Give the key
the state `hidden: true`, which keeps it out of the game until a script
reveals it:

```yaml
  small_key:
    name: small key
    synonyms: [key]
    in: kitchen
    traits: [portable]
    state: { hidden: true }
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
    on:
      search: |
        if world.small_key.hidden then
          world.small_key.hidden = false
          world.small_key:move_to(player)
          say("You lift the corner of the mat. Underneath is a small key, which you take.")
        else
          say("Just dust.")
        end
```

- **`${if ...}...${else}...${end}`** makes text react to the game. Here the
  lumpy corner disappears once the key is found. Inside a template, `self`
  is the object the text belongs to. If an object's own description already
  talks about it being open or locked this way, the automatic sentence is
  left out, so nothing is said twice.
- **`world.<id>`** gives scripts access to any object. Its state fields can
  be read and written directly.
- **`search`** replaces the built-in SEARCH for this one object. Players can
  trigger it with `search mat`, `look under the mat` or `look under rug`.
- **`say(...)`** prints text, and **`:move_to(player)`** puts the key in the player's hands.

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

**Settings:** `describe_state` (default `true`) adds open, closed, locked
and worn to examine text, room descriptions and lists.

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
