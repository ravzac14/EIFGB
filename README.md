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

> **Status: early.** Games run from the command line, and in the browser
> with the web player (in `web/`), where a visual editor draws your map of
> rooms, exits and doors, places your objects and characters, and edits
> scripts, timers, endings and conversations.
> The file format may still change before 1.0.

- [Quick start](#quick-start)
- [Tutorial: Midnight Snack](#tutorial-midnight-snack)
- [States and descriptions](#states-and-descriptions)
- [Containers and supporters](#containers-and-supporters)
- [Timed events](#timed-events)
- [Characters: NPCs and enemies](#characters-npcs-and-enemies)
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

In this tutorial you build a small game in seven steps: a hungry player, a
locked pantry, a key hidden under a mat, an apple, and a pie that's too hot
to touch until it cools. The finished game
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
  locked.", and once the player has tried the door, the kitchen mentions
  "The pantry door is locked." Things that are listed get a tag instead,
  like "a wooden box (open)", once the player has examined or handled
  them. The same
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
  several actions reveal it. Or use an action category, such as
  `revealed_by: (physical action) mat`, so that kicking, pulling, dragging,
  lifting or pushing the mat all work.
- **`${if ...}...${else}...${end}`** makes text react to the game. Here it
  gives players a clue, and the lumpy corner disappears once the key is
  found. Inside a template, `world.<id>` is any object and `self` is the
  object the text belongs to. Players can use the words of a description
  too: `x lumpy corner` examines the mat, because its description
  mentions one. If an object's own description already talks
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

### Step 6: a pie that has to cool

Timers make things happen after a number of actions. Use one to give the
player a second snack, if they're patient: a pie that's too hot to touch
until it has cooled. Add it to `objects`:

```yaml
  pie:
    name: apple pie
    synonyms: [pie]
    in: kitchen
    traits: [portable, edible]
    state: { steaming: true }
    description: A golden apple pie, fresh from the oven.
    on:
      before_take: |
        if self.steaming then
          return block("Ow! It's far too hot to pick up. Give it a few minutes.")
        end
      before_eat: |
        if self.steaming then
          return block("You'd burn your tongue. Let it cool first.")
        end
      after_eat: end_game("feast")
```

Then add a `timers` section that cools it, and an ending for eating it:

```yaml
timers:
  pie_cools:
    after: 6
    do: world.pie.steaming = false
    say: You catch the smell of apple pie. It must have cooled by now.

endings:
  # ...fed stays as before
  feast:
    kind: win
    text: Warm apple pie at midnight. Nothing could be better.
```

- **`steaming`** is a state you made up. The game describes it for you:
  `x pie` says "It's steaming.", and from then on the kitchen lists "an
  apple pie (steaming)".
- **`before_take`** and **`before_eat`** run before the built-in verbs.
  Returning `block(...)` stops the verb and prints the message instead.
  Once `steaming` is false they return nothing, so taking and eating work
  as normal.
- **`after: 6`** fires the timer once, six actions into the game. Any
  command that takes a turn counts, including ones that fail, like trying
  to take the hot pie. Commands the game didn't understand, and SAVE, UNDO
  or SCORE, don't count.
- **`do`** runs Lua when the timer fires, and **`say`** prints a message.
  The message is shown wherever the player is, so it's worded to make
  sense from the pantry too.

Play it: `take pie`, `eat pie`, then `wait` until the smell reaches you.

```
> take pie
Ow! It's far too hot to pick up. Give it a few minutes.

> eat pie
You'd burn your tongue. Let it cool first.

> z
Time passes.

> z
Time passes.

> z
Time passes.

> x pie
A golden apple pie, fresh from the oven. It's steaming.
You catch the smell of apple pie. It must have cooled by now.

> x pie
A golden apple pie, fresh from the oven. It isn't steaming.

> take pie
Taken.
```

A timer can also wait for the player to start it, with `started_by: light
fuse` for example, or repeat with `every:`.
[Timed events](#timed-events) has the details.

### Step 7: a walkthrough file

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

In the web editor, you don't have to write it by hand: play your game
from the start in the **Test play** tab, then press **Save as
walkthrough**. From then on, the editor replays it after every change. It
shows "✓ The walkthrough still wins" in Test play, or a warning under
Problems that says where it stopped working. The walkthrough is
`walkthrough.txt`, in the Files tab with the rest of your game. Test play
always uses the same random numbers as the check, so what you recorded
plays the same way again.

That's a complete game. Compare yours with
[`spec/examples/midnight_snack/game.yaml`](spec/examples/midnight_snack/game.yaml).

Ready for more code? The [scripting tutorial](docs/SCRIPTING.md) builds a
campfire game with Lua handlers, custom verbs, a script file, variables,
timers you control from code, and random events.

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
| `hidden` | `revealed_by`, or scripts | Out of the game: it can't be seen or referred to until revealed. A hidden door is a secret door: its exits aren't listed or usable until it's found. On a door, `hidden` can name the side it's hidden from (`hidden: library`), like `blocked`. |
| `blocked` | scripts, on `door`s | Held shut: barred, bolted or wedged. `true` means from both sides; a room ID (`blocked: cellar`) or a list of room IDs means only from those sides. A blocked door is always closed. Opening it from an unblocked side lifts the block. |

### What players see

With nothing more than the declarations above, playing gives:

```
Garden Shed
A cramped wooden shed.
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

> open cupboard
You open the cupboard.

> look

Garden Shed
A cramped wooden shed. The cupboard is open and empty.
```

State is mentioned in three places:

| Where | What's added | Example |
|---|---|---|
| Examining a thing | Every state, after its description | "It isn't lit. Its oil is 3." |
| Lists (room contents, inventory, inside containers) | Tags for states that are "on", once the player knows them | "a brass lantern (lit)", "an iron gate (locked)", "a small box (empty)" |
| Room descriptions | A sentence for things that aren't listed (scenery), for "on" states, once the player knows them | "The cupboard is closed.", "The cupboard is open and empty." |

Lists and rooms leave out "off" states and numbers, because "a lantern
(not lit, oil: 3)" in every list would be clutter. Examine the thing to
see everything.

**Tags and room sentences show what the player has found out.** A thing
gets tags in lists, and scenery gets its sentence in the room
description, only after the player has examined, searched or looked in it, or handled
it themselves: opened, closed, locked, unlocked, worn, taken off, emptied,
walked through (doors), put something in or on it, or taken something
from it. Things the player starts out carrying count as known. After
that, tags always show the current state.

```
Dining Room
A simple dining room in a log cabin. At the table there's a single chair.
On the simple, wooden table you can see a small box.

> x small box
It's empty.

> look

Dining Room
A simple dining room in a log cabin. At the table there's a single chair.
On the simple, wooden table you can see a small box (empty).
```

Two tags come from what's inside rather than from state:

- **`empty`**: a container the player can see into, with nothing visible
  in it. Hidden things don't count, so a box with a secret compartment
  looks empty, unless its description hints at what's hidden with
  `${if world.<id>.hidden}`. Then the hint is left to speak for itself.
- **`heavy`**: something the player could carry on its own, but not with
  everything in it (see [Containers and supporters](#containers-and-supporters)).
  It only applies when the player has a carrying capacity.

Examining says the same things as sentences: "It's empty." and "It's too
heavy to lift with everything in it."

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
| `state.empty`, `state.heavy` | Examining | "It's empty." / "It's too heavy to lift with everything in it." |
| `state.tag.open`, `.closed`, `.locked`, `.worn`, `.empty`, `.heavy` | Lists | "open" … "heavy" |
| `state.here.open`, `.closed`, `.locked` | Room descriptions | "${The(args.target)} is open." … |
| `state.custom.on`, `state.custom.off` | Examining your true/false fields | "It's ${args.state}." / "It isn't ${args.state}." |
| `state.custom.value` | Examining your number and text fields | "Its ${args.state} is ${args.value}." |
| `state.here.custom.on` | Room descriptions, your fields | "${The(args.target)} is ${args.state}." |
| `state.here.several` | Room descriptions, when one thing has several states | "${The(args.target)} is ${args.list}." (the tags, joined: "open and empty") |

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

## Containers and supporters

Bags, boxes and jars are **containers**: things go *in* them. Tables and
shelves are **supporters**: things go *on* them. Put things there with `in:`:

```yaml
objects:
  table: { name: table, in: hall, traits: [supporter, scenery] }
  bag:   { name: cloth bag, in: table, traits: [portable, container, openable], capacity: 2 }
  coin:  { name: coin, in: bag, traits: [portable] }
  case:  { name: glass case, in: hall, traits: [container, openable, transparent, scenery] }
  jewel: { name: jewel, in: case, traits: [portable] }
  wardrobe: { name: wardrobe, in: hall, traits: [container, openable, enterable, scenery], size: 5 }
  bed:   { name: bed, in: hall, traits: [supporter, enterable, scenery] }
```

| Trait | Effect |
|---|---|
| `container` | Holds things in it. Without `openable` it's always open (a jar, a bowl). |
| `supporter` | Holds things on it, always visible. |
| `openable`, `lockable` | Can be opened and closed, locked and unlocked (with `unlocks_with:`). A closed container hides what's inside. |
| `transparent` | A closed container whose contents can be seen but not reached: "You can't reach the jewel: the glass case is closed." This applies to every verb that touches things, including your own and KICK, PULL and the other try-it verbs, and their handlers don't run. EXAMINE, SEARCH and LISTEN work through the glass, and so does any verb you mark `reach: false`. |
| `flammable` | Can be set alight (SET IT ALIGHT, BURN, LIGHT, with something burning in hand). See [Tricks players can find](#tricks-players-can-find). |
| `destructible` | Can be smashed (HIT, SMASH, BREAK), spilling what's in it, and burns away completely if it's `flammable` too. `destructable` is accepted as well. |
| `enterable` | The player can get in it (a container) or on it (a supporter): GET IN, CLIMB INTO, SIT ON, STAND ON, CLIMB ON, LIE ON, then GET OUT, GET OFF or STAND UP. |
| `pushable` | Too heavy to carry, but can be pushed into the next room: PUSH CRATE NORTH (or SHOVE, DRAG, PULL, ROLL, SLIDE, MOVE). See [Pushing and climbing](#pushing-and-climbing). |

What players can do and see:

```
> look
...
On the table you can see a cloth bag and a small box.
In the small box you can see a ring.

> take coin from jar
The coin isn't in the glass jar.

> i
You're carrying a cloth bag (open) and a small box.
In the cloth bag is a coin.
In the small box is a ring.

> put all in bag
> empty bag                  (onto the floor)
> empty jar into box         (or onto a supporter)
> take all from box
```

- **LOOK and EXAMINE** describe contents as deep as the player can see.
  The inventory lists what's carried, then a sentence for what's inside
  each carried container.
- **Getting in or on something** shows in the room title: "Hall (in the
  wardrobe)", "Hall (on the bed)".
  - Closing an `enterable` container from inside shuts the player in, and
    they can't see out: "You're shut inside the wardrobe."
  - Walking away, or climbing into something else, gets out first: "(first
    getting off the bed)". OUT gets out too.

**Size, weight and capacity.** Every thing has a `size`, 1 by default.
Its **weight** is its size plus the weight of everything in or on it, so
a size-2 crate holding a size-2 brick weighs 4. A container, supporter or
the player can have a `capacity`, which limits the total weight of what's
directly in or on it (or carried). Without a `capacity`, nothing bigger
than the holder itself fits, so a piano never goes into a jar.

When something is small enough but too heavy because of what's in it,
TAKE says "The crate is too heavy to lift with everything in it." Once the
player has looked at it, lists tag it "(heavy)". Taking things out makes
it lighter.

**Carrying capacity.** The player's own `capacity` limits the total weight
they can carry, including what's inside the things they hold:

```yaml
player:
  capacity: 10     # total size they can carry
```

| `capacity` | The player can carry |
|---|---|
| left out, `none` or `~` | anything (the default) |
| `0` | nothing at all: TAKE says "You can't carry anything." |
| a number | things up to that total weight. Too much gives "You're carrying too much already.", and one thing bigger than the whole capacity gives "The boulder is too big to carry." |

`check` warns if the player starts out carrying more than their capacity.

Scripts can change a capacity during play, the player's or a container's.
A backpack that lets the player carry more:

```yaml
  backpack:
    name: backpack
    in: hall
    traits: [portable, wearable]
    on:
      after_wear: |
        player.capacity = 15
        say("With the backpack on, you can carry much more.")
      after_take_off: player.capacity = 10
```

`player.capacity` reads the current limit, or `nil` when there's none.
Set it to a whole number, or to `nil` (or `"none"`) to remove the limit.
For a container, `nil` brings back the "nothing bigger than itself" rule.
Lowering a capacity never drops anything; it only stops more from being
added. Changes are saved and undone like everything else. Because
`capacity` has this meaning, it can't be used as the name of a state
field.

```
> put piano in jar
The piano is too big for the glass jar.
> take boulder
The boulder is too big to carry.
> take sack
You're carrying too much already.
```

### Pushing and climbing

Something big and heavy can be `pushable`: the player can't pick it up,
but can push it into the next room, and follows it in. And anything can be
`high: true`, out of reach until the player stands on something.
Together they make the classic puzzle: push the crate under the shelf,
climb on, and take the key.

```yaml
rooms:
  shed: { name: Shed, exits: { north: { to: barn, through: shed_door } } }
  barn: { name: Barn, exits: { south: { to: shed, through: shed_door } } }
objects:
  shed_door: { name: shed door, in: shed, traits: [door, openable, scenery] }
  crate: { name: wooden crate, synonyms: [crate], in: shed, traits: [supporter, enterable, pushable] }
  shelf: { name: high shelf, synonyms: [shelf], in: barn, high: true, traits: [supporter, scenery] }
  key:   { name: brass key, in: shelf, traits: [portable] }
```

```
> take crate
The wooden crate is too heavy to carry, but you could push it somewhere: PUSH IT NORTH, say.

> push crate north
(first opening the shed door)
You push the wooden crate north.
Barn
...

> take key
The brass key is up out of reach. You'd need something to stand on.

> climb on crate
You get onto the wooden crate.

> take key
Taken.
```

- **Pushing** goes just like walking: a closed door is opened on the way,
  and a locked door, a blocked exit or a `when` that fails stops it, with
  the crate left where it was. The thing has to be standing in the room
  (not on or in something else), and the player can't be on it. PUSH
  CRATE with no direction is the ordinary PUSH, for your own handlers.
- **Up high:** a `high` thing, and everything on or in it, can be seen
  and examined, but nothing that needs to touch it works ("The brass key
  is up out of reach."), including your own verbs and handlers, until the
  player is on a supporter: CLIMB ON THE CRATE, STAND ON THE CHAIR. What
  the player carries is always in reach.
- Messages to reword: `push.done`, `push.cant`, `push.carried`,
  `push.on_it`, `push.stuck`, `take.pushable` and `reach.high`.

## Timed events

Timers make things happen after a number of **actions**. Any command
that takes a turn counts. Commands the game didn't understand, and
out-of-game commands like SAVE, UNDO and SCORE, don't.

```yaml
timers:
  ship_closer:
    after: 40                        # once, after 40 actions
    when: not vars.ship_contacted    # skipped if this is false when it's due
    say: Out at sea, the ship's lights swing dangerously close.
  shipwreck:
    after: 60
    when: not vars.ship_contacted
    end: lost                        # ends the game with the `lost` ending

  fuse:
    after: 3
    started_by: light fuse           # starts when the player does this
    stopped_by: (physical action) fuse
    say: The fuse burns down...
    end: boom

  drip:
    every: 5                         # repeats until stopped
    started_by: turn tap
    stopped_by: push tap
    say: Water drips from the tap.
```

| Field | Meaning |
|---|---|
| `after: N` / `every: N` | Fire once N actions after starting, or every N actions. Exactly one is required. |
| `started_by`, `stopped_by` | Rules like `revealed_by`: `"<verb> <object>"`, `"(<category>) <object>"`, or a list. Starting a timer that's already running doesn't reset it. |
| `running` | Whether it counts from the start of the game. Defaults to `true`, or `false` if it has `started_by`. |
| `when` | A Lua condition checked when it's due. If false, this firing is skipped. |
| `say`, `do`, `end` | What happens: print text, run Lua, end the game. Any combination. |
| `noise: { at, loudness, faction }` | A noise that hunters within `loudness` rooms (of `faction` only, if given) come to investigate. `at` is a room, or a thing (the noise is wherever it is); without it, wherever the player is. See [Tricks players can find](#tricks-players-can-find). |

A timer started by an action counts the actions *after* that one. With
`after: 3`, lighting the fuse and then waiting three times gives the
explosion on the third wait. Timers are saved and undone with everything
else.

Scripts can also use `start_timer("bell")`, `stop_timer("bell")` and
`timer_left("bell")`. `timer_left` gives the number of actions left,
counting the current one, or `nil` if the timer isn't running.

## Hints

Hints help players who are stuck, one step at a time. List them by
puzzle, with a nudge first and the answer last:

```yaml
hints:
  find_key:
    when: world.small_key.hidden       # offered while this is true
    steps:
      - The pantry door needs a key. Where might one be hidden?
      - Have a closer look at the mat.
      - SEARCH THE MAT.
  open_door:
    when: not world.pantry_door.open
    steps:
      - UNLOCK DOOR, then OPEN DOOR.
```

HINT shows the next step of the **first** puzzle in the list whose `when`
is true (a puzzle without `when` is always offered): "(Hint 1 of 3) The
pantry door needs a key…". Typing HINT again gives the next step, and once
they've all been shown, the last one repeats. So put puzzles in the order
players meet them, and write each `when` to be true from when the puzzle
can be found until it's solved. Hints take no turn, and are saved with the
game. Steps are text, so `${…}` works in them.

The editor's starter templates all have hints to copy from.

## Characters: NPCs and enemies

Characters take a turn after every action the player takes (the same
actions that count for timers). What they do depends on their
`behavior`:

| `behavior` | What it does each turn |
|---|---|
| `stationary` | Nothing. The default for a character without `talk`. |
| `conversationalist` | Nothing, so the player always knows where to find them. The default for a character with `talk`. |
| `wanderer` | Moves to a random neighbouring room, with a `wander` percent chance (default 50). |
| `merchant` | Trades with the player (see below). Wanders (default 25%), but never walks off while the player is in the room. |
| `hunter` | Attacks the player if they're in the same room. Otherwise heads one room closer to them, if they're within `range` rooms (default: anywhere), or else wanders (default 25%). |
| `guard` | Attacks the player in its own room, and otherwise stays put. |
| `follower` | Follows the player from room to room. |
| `defender` | Never starts a fight, but attacks anyone who attacked it (at once: its `patience` is 0), when they're in the same room. Otherwise stays put. |

```yaml
player:
  health: 30
  money: 20

objects:
  sword: { name: sword, in: hall, traits: [portable], damage: 4 }
  lamp: { name: brass lamp, synonyms: [lamp], in: peddler, traits: [portable], price: 8 }

characters:
  wolf:
    name: grey wolf
    synonyms: [wolf]
    in: woods
    behavior: hunter
    health: 8
    damage: 3
  peddler:
    name: peddler
    in: shop
    behavior: merchant
    money: 5
```

Settings that shape a behaviour:

- **`wander`**: percent chance of moving on a turn with nothing better to
  do. `wander: 0` keeps a merchant or hunter in one place.
- **`area: [market, square]`**: the rooms a character keeps to.
- **`range: 3`**: how many rooms away a hunter or follower notices the player.

Characters use any exit that isn't conditional (`when:`), and slip
through closed doors as long as they aren't locked, blocked or hidden
from their side. The player sees them come and go: "The peddler goes
north.", "The grey wolf arrives."

### Fighting

Give the player and anything that can be hurt a `health`, and anything
that hits a `damage` (default 1). An object with `damage` is a weapon.

```
> z
Time passes.
The grey wolf arrives.

> attack wolf with sword
You hit the grey wolf with the sword.
The grey wolf attacks you.

> look

Hall
You can see a grey wolf (badly wounded) here.
Known exits: north and east.
The grey wolf attacks you.

> kill wolf with sword
You hit the grey wolf with the sword.
The grey wolf is defeated.

> health
You're hurt: 12 out of 30.
```

- **Attacking:** ATTACK, HIT, KILL, FIGHT, STRIKE and STAB, optionally
  WITH a weapon the player is holding. Without one, the player's own
  `damage` is used (`player: { damage: 2 }`, default 1). Hitting
  something without health still gets "Violence isn't the answer here."
- **Enemies** (hunters and guards) hit back with their best weapon, or
  their own `damage`.
- **Self-defense and patience:** everyone except pacifists (below) fights
  back eventually. A
  character's `patience` is how many attacks it puts up with first:
  none for hunters, guards and defenders, and 2 for everyone else. Until
  it runs out, the player gets a warning:

  ```
  > hit monk
  You hit the monk.
  The monk is losing patience with you.

  > hit monk
  You hit the monk.
  The monk is losing patience with you.

  > hit monk
  You hit the monk.
  The monk attacks you.
  ```

  After that, the attacker is one of the character's `foes`, and it
  attacks them whenever they're in the same room. It doesn't chase a foe
  who leaves, but it doesn't forget either. Set `patience: 0` for a
  merchant who won't be robbed, a higher number for a saintly one, or
  `fights_back: false` for someone who never fights at all. Scripts can
  read and change both: `self.patience = 2` calms a character down, and
  `self.foes = nil` forgives everyone.
- **Wounds** show once the player has seen them: "a grey wolf (wounded)",
  then "(badly wounded)" at half health or less.
- **Defeat:** at 0 health a character drops what it carries and leaves
  a body (see [Factions and corpses](#factions-and-corpses)). An
  `on: { defeated: ... }` handler on the character runs first. Returning
  `block("...")` replaces the usual message and the dropping. A handler
  can also heal the character or move it away: the wolf could limp off
  instead.
- **Death:** when the player's health reaches 0, the global `died`
  handlers run, and then the game ends with the `died` ending. Write
  your own `died` ending to change its text, or heal the player in the
  handler (`player.health = 5`) to save them.
- **HEALTH** (or DIAGNOSE) tells the player how they're doing, and
  doesn't take a turn, so it never gives an enemy a free hit.

### Factions and corpses

Give enemies sides, and they'll fight each other as well as the player:

```yaml
factions:
  wolves: { hostile_to: [goblins, player], eats_corpses: true }
  goblins: { hostile_to: [wolves, player] }

characters:
  alpha: { name: alpha wolf, in: den, faction: wolves, behavior: hunter, health: 10, damage: 3 }
```

- **Who they attack:** hunters and guards attack anyone their faction is
  `hostile_to`, including the player if `player` is listed. Without a
  faction, they attack only the player.
- **Who they pick:** a hunter goes for the nearest enemy, and the player
  wins a tie. So monsters in a room with the player all attack the
  player, but leave the room and they turn on each other. Fights and
  feeding next door can be heard: "You hear snarls and the clash of a
  fight somewhere north."
- **Corpses:** a defeated character drops what it carries and leaves a
  body: "a goblin chief (dead)". Players can call it a corpse, body,
  remains, carcass or bones. `corpse: false` makes a character vanish
  instead.
- **Attitude:** each faction has an attitude toward the player, as they
  present themselves (as themselves, or disguised as some faction). It
  starts at 0 toward a side the faction is hostile to, and at the
  faction's `patience` (default 2) otherwise, including toward a player
  disguised as one of its own. Each member the player provokes into a
  foe (by using up that member's own patience) lowers it by one, but only
  if a member of the faction **witnesses** it: one in the room, alive, who
  can see the player. A victim that survives the blow is a witness; one
  killed outright with nobody else around can't tell anyone, so a silent
  kill leaves the player's reputation clean. Once seen, the news spreads
  at once: "The villagers are growing wary of you." At 0, every member attacks the
  player on sight, whatever its behaviour: "The villagers turn against
  you.", or in disguise, "The goblins see through your disguise!" Putting
  the disguise back on doesn't help, and it doesn't recover by itself:
  only an apology does (below).
- **Apologies:** APOLOGISE TO MILLER (or SAY SORRY TO, MAKE PEACE WITH,
  BEG … FOR FORGIVENESS, CALM … DOWN). If the character accepts, it drops
  its grudge, its patience is restored, and its faction's attitude goes
  up by one ("The villagers seem a little less wary of you."), never
  above where it started. Each character accepts `forgiveness` apologies
  over the whole game (default 1; `0` never forgives): "The baker has
  heard enough of your apologies." Anyone hostile to the player by
  nature, like a wolf pack, "isn't interested in apologies." Apologising
  in disguise mends the disguise's standing, one apology at a time.
- **Pacifists** never fight: not with their faction, and not back.
  Merchants and conversationalists are pacifists by default, so the
  village shopkeeper doesn't pick up a cleaver when the village turns on
  you. Mark anyone else with `pacifist: true`, or set `pacifist: false`
  for a merchant who fights. Hurting a pacifist still counts with its
  faction: "The villagers are growing wary of you."
- **Invincible** characters (`invincible: true`) can't be hurt: "The
  oracle shrugs off the blow." Use it for characters the story can't do
  without. They can still lose patience and fight back, unless they're
  also pacifists.
- **Scavengers:** a faction with `eats_corpses` eats bodies in its room
  when there's no one to fight, and hunters go looking for them.

### Tricks players can find

Nineteen features reward players who try something clever:

- **Fire:** SET THE TENT ALIGHT (BURN, SET FIRE TO, TORCH, or LIGHT for
  anything `flammable`) with something burning in hand: a lit torch or
  lamp. "You touch the torch to the tent, and it catches fire." A fire
  lights the room, sends anything that `fears: [fire]` running, and each
  turn spreads to one more flammable thing in the room (or inside it).
  After three turns it burns out: a `destructible` thing burns away to
  ash, spilling what was in it ("Out of what's left of the tent falls a
  key."); anything else is just left. Burning things are plain to see:
  "The tent is burning.", "a crate (burning)".

  With nothing left to catch in its room, a fire leaps through an open
  way (not a shut door) into the next: "The flames leap east, into the
  next room." Everyone in a burning room is scorched each turn ("The
  flames scorch you!"), sleepers wake, and a character killed by a fire
  the player started counts as the player's kill: "The goblin has been
  burnt to death." Smoke drifts into neighbouring rooms ("Smoke drifts in
  from the east."), and peeking shows "Flames are raging in there."

  PUT OUT smothers a small (portable) fire by hand; anything bigger is
  "burning too fiercely to put out with your bare hands". Objects with
  `douses: true` (a bucket of water, a handful of sand) put any fire out,
  and are used up: POUR BUCKET ON HAY, THROW SAND AT HAY, EXTINGUISH HAY
  WITH BUCKET.
- **Smashing:** HIT, SMASH or BREAK something `destructible` (with no
  health) and it's destroyed, spilling its contents: "You smash the barrel
  to pieces."

- **Traps:** an object with `trap: { holds: 3 }` (and optional `damage`)
  can be SET (ARM, LAY) wherever the player is: "a snare (armed)". The
  first character to walk into that room is caught ("The wolf is caught in
  the snare!"), held for `holds` turns, tagged "(caught)", and can only
  lash out at an enemy right next to it, then breaks free. Any damage
  counts as the player's blow (`kills: caught in a snare` tells the
  story). Bait left by a trap leads hungry hunters into it. The player
  never sets off their own traps, but one an author placed (armed, and
  perhaps `hidden`) catches them: "You're caught in the hidden pit!", and
  they can't move until they work free. DISARM makes a trap safe; taking
  an armed one disarms it first. Characters that leave no tracks (ghosts,
  birds) pass over traps.

- **Fear and morale:** a character or faction with `morale: 40` flees its
  enemies once it's below 40% of its health, or when it's outnumbered in
  the room (more enemies than allies): "The wolf flees north!" With
  `fears: [torch]` (object IDs, or `fire` for anything burning), BRANDISH
  TORCH (WAVE, SHOW … TO, THRUST … AT) makes it panic ("The wolf shrinks
  back in fear!") and run for three turns, tagged "(frightened)". A
  frightened or badly hurt character keeps away rather than hunting; one
  with nowhere to run fights. A companion in that state is "too scared"
  to attack on orders. Without `morale`, characters never flee.

- **Giving orders:** tamed characters, and any with `obeys: true` (a
  hireling, a soldier), take orders: TELL REX TO … (or ORDER, ASK,
  COMMAND, INSTRUCT), or just REX, ….

  | Order | Does |
  |---|---|
  | ATTACK / KILL / GET the goblin | Goes after it, even into another room, until it's dead or out of reach. |
  | STAY / SIT / WAIT / GUARD | Stays put (still fighting the player's enemies that come to it). |
  | FOLLOW / COME / HEEL | Back to following the player. |
  | GO NORTH (or just NORTH) | Goes that way, then stays there. |
  | FETCH / BRING the stick | Fetches it. |
  | DROP IT | Drops what it's carrying. |

  "The wolf pup goes for the goblin!" Others "ignore you"; a pacifist
  won't fight; nobody will attack the player; and anything else gets "Rex
  looks at you blankly", with a hint (which doesn't take a turn).

- **Eavesdropping:** characters with `chatter:` talk among themselves
  when they have someone awake to talk to and can't see the player.

  ```yaml
  characters:
    chief:
      name: goblin chief
      in: camp
      chatter:
        - "The key's under my pillow, same as always."
        - { say: "WHERE IS MY SUPPER?", loud: true }
        - { say: "I'm not asleep.", repeat: true, when: "vars.night" }
  ```

  At most one line is said per room each turn, in order. Hidden in the
  same room (in the barrel, or unseen in the dark), the player overhears
  everything: "The goblin chief says to the goblin grunt, "…"". From the
  next room they hear `loud` lines ("You hear a voice from the north:
  "WHERE IS MY SUPPER?""), and quiet ones too while they LISTEN AT THE
  DOOR, LISTEN NORTH or EAVESDROP ("You hear nothing." if no one speaks).
  Characters in plain view of the player hush. A line is done with once
  the player has heard it (unless it `repeat`s), so a clue nobody heard
  comes round again.

- **Pickpocketing:** STEAL KEY FROM GUARD (also TAKE … FROM, PICKPOCKET …
  FROM, LIFT, PINCH), or PICKPOCKET GUARD / ROB GUARD for the most
  valuable thing it carries. It works if the victim is asleep, busy eating
  (bait), or fooled by the player's disguise (unless it already has a
  grudge): "You slip the bead away from the goblin without being
  noticed." Worn things only come off a sleeper. Otherwise: "The goblin
  catches you reaching for the bead!" Nothing is taken, and it counts as
  provoking the victim (patience, foes, and the faction, if anyone saw).
  A merchant who catches the player won't trade with them until they
  apologise.

- **Darkness:** a room with `dark: true` can only be seen in by the light
  of a `light_source` that's lit (`lit` or `on`) and in plain sight:
  lying there, carried by the player or anyone else, but not shut in a
  bag. In the dark, LOOK gives "Darkness / It's pitch dark. You can't
  see a thing.", the player can only handle what they're holding
  ("It's too dark to see."), and characters are just "Something":
  "Something attacks you." Moving still works, to feel your way out.
  LIGHT LAMP (TURN ON, SWITCH ON, IGNITE) and EXTINGUISH LAMP (PUT OUT,
  BLOW OUT, TURN OFF, SNUFF) are standard verbs for light sources;
  lighting one in the dark shows the room at once.

  Characters are as blind in the dark as the player, unless they have
  `sees_in_dark: true` (or their faction does). A blind character in an
  unlit room can't see the player to attack or hunt them, witness what
  they do, or find a body; the moment the player lights a lamp, it can.
  A monster with `sees_in_dark` hunts happily in pitch darkness.

  The player can see in the dark too: with `player: { sees_in_dark: true }`,
  a script setting `player.sees_in_dark = true` (a potion), or by wearing
  something with `sees_in_dark: true` (night goggles). And when the player
  is shut inside something (a wardrobe, a barrel), a lamp in there with
  them lights it, even in a dark room.

  Light shows through open ways between rooms: through an exit with no
  door, or an open one. LOOK in a dark room next to a lit one adds
  "Light spills in from the north." (or "from above"), and LOOK in a lit
  room next to a dark one adds "It's dark to the north." Neither makes
  anything visible. To reword them, override the messages
  `look.light_from` and `look.dark_beyond`, and the phrases they list
  (`look.from`, `look.toward`, and `look.from_up`, `look.toward_down` and
  so on for up, down, in and out).

- **Peeking:** PEEK THROUGH THE DOOR, PEEK NORTH, PEER NORTH or LOOK
  NORTH gives an impression of the next room without going in or being
  noticed: never its description, just whether anyone's there (and
  asleep), a body, how much is lying about, and whether a light is
  burning. The room's name is given only if the player has been there.

  ```
  > peek through the door
  You peer through the oak door. Two figures are in there. One of them seems to
  be asleep.

  > peek down
  You peer down. It's too dark to make anything out.
  ```

  A dark room (below) shows nothing.

- **Sleep and sneaking:** a character with `asleep: true` in its state (or
  put to sleep by a timer or script: `world.guard.asleep = true`) does
  nothing and notices nothing: no attacks, no hunting, no finding bodies,
  no trading or talking ("The guard is fast asleep."). It's tagged
  "(asleep)". It wakes when the player walks in normally, when a noise
  reaches it, or when a fight breaks out in its room: "The guard wakes
  with a start!" SNEAK NORTH (or CREEP, TIPTOE) moves without waking
  anyone. Hitting a sleeper is a sneak attack, for double damage: "You
  catch the guard asleep!"

  ```yaml
  timers:
    night:
      after: 20
      say: The guard yawns and nods off.
      do: world.guard.asleep = true
  ```

- **Hiding bodies:** a silent kill stays secret only until someone finds
  the body. A faction member that comes across the body of one of its
  own that the player killed, in plain sight, raises the alarm: "The
  watchman finds the body of the miller, and raises the alarm!" (or, from
  next door, "You hear a cry of alarm somewhere north."). The faction
  sours as if it had seen the kill, and hunters within two rooms come
  running. Bodies can be carried (TAKE BODY), though one in the player's
  arms is in plain sight too. Put one somewhere closed (HIDE BODY IN
  CHEST works on a body lying on the floor) or BURY BODY in a room with
  `diggable: true`, and nobody finds out. Scavengers that eat a body
  destroy the evidence too. Bodies killed by someone else don't count
  against the player.

- **Tracks:** everyone who walks out of a room leaves prints there.
  EXAMINE TRACKS (LOOK FOR TRACKS, X FOOTPRINTS…) reads them, freshest
  first: "Fresh paw prints lead east. Your own footprints lead south."
  FOLLOW TRACKS walks the way the freshest prints lead (never your own),
  and FOLLOW PAW PRINTS or FOLLOW THE WOLF picks whose. Prints are
  `footprints` unless a character or its faction says otherwise
  (`prints: paw prints`); ghosts and birds can have `leaves_tracks: false`.
  They fade after `settings.track_life` actions (default 30; `0` turns
  tracks off). A room with `trackless: true` (a stream, bare rock) holds
  none. A hunter with `tracker: true` that loses sight of the player
  follows their fresh tracks, so wading through a stream is how to throw
  it off.

- **Naming things:** give an object or character `nameable: true`, and
  CALL THE DOG REX (also NAME … (AS), RENAME … (TO), CHRISTEN) gives it a
  name: "From now on, the old dog is called Rex." It's a proper name from
  then on ("Rex arrives.", "You're carrying a stick and Sting."),
  including in `${The(self)}` templates, and the player can use it as
  well as the old words. Names are tidied to up to three capitalised
  words, can be changed again, and are saved and undone like everything
  else. In Lua, `x.name` is the current name, `x.proper` says whether the
  player named it, and `x:rename("Rex")` names it.

- **Throwing:** THROW (HURL, TOSS, CHUCK) X AT Y attacks from across the
  room with X's `damage` (or the player's own, for anything that isn't a
  weapon), and X lands in the room. At something that can't be hurt, it
  bounces off. THROW X NORTH lands it in the next room, even over an
  exit the player can't walk through, like a chasm whose `when:` is
  false (though not through a closed door), and where it lands it makes
  a small noise that draws hunters next door. THROW X just tosses it
  aside. Anything weighing more than 3 is too heavy to throw. A tamed
  companion or follower with `fetches: true` bounds after whatever the
  player throws, if it can get there, and brings it back:

  ```
  > throw stick east
  You throw the stick east, and it lands somewhere out of sight.
  The dog bounds after the stick.
  The dog goes east.
  ...
  The dog drops the stick at your feet.
  ```

- **Taming, gifts and bribes:** GIVE (HAND, OFFER, FEED) something to a
  character.
  - `tame_with: [bone]`: giving it tames the character ("The wolf pup
    wolfs down the bone, and seems to have decided you're a friend."; food
    its faction eats is eaten). A tamed character follows the player
    anywhere, fights anyone who'd fight the player, and never attacks the
    player, unless the player attacks it.
  - `likes: [flowers]`: a welcome gift ("The aunt gladly accepts the
    flowers."). It works like an apology, but isn't limited by
    `forgiveness`.
  - `bribe: 5`: GIVE 5 COINS TO GUARD (or PAY 5 TO GUARD, BRIBE GUARD
    WITH 5) buys the character off: "The guard pockets 5 coins and looks
    the other way." It won't attack the player again, even if its
    faction turns, unless the player attacks it. Too little: "The guard
    scoffs. It'll take more than 3 coins."
  - Anything else: "The wolf pup doesn't want the pebble." Asking "how
    much?" or "to whom?" doesn't take a turn.

- **Making noise:** SHOUT (or YELL, SCREAM, MAKE NOISE) draws hunters
  that can reach the player within 2 rooms (`player: { voice: 3 }`
  carries further). They come to investigate whatever their `range`,
  search, and give up if they find nobody, so a player can shout from
  inside the barrel and let two packs meet outside. A timer can make a
  noise too, like a bell that rings every few actions:

  ```yaml
  timers:
    bell:
      every: 10
      say: The temple bell tolls.
      noise: { at: belfry, loudness: 3 }
  ```

  In Lua, `make_noise(world.belfry, 3)` or `self:make_noise(3)` (for
  BANG POT or RING BELL).

  **Mimicry:** a noise can call just one faction. Give a faction
  `calls: [howl]`, and HOWL (or IMITATE A WOLF, MIMIC A GOBLIN: a
  faction's name or its members' names work too) draws only its hunters,
  as far as the player's voice carries: "You let out a convincing howl."
  Built in: howl, bark, growl, roar, hoot, screech, squawk, caw, grunt and
  bleat. An object with `calls: { faction: goblins, loudness: 2 }` does
  the same when the player BLOWs, SOUNDs, PLAYs, RINGs or TOOTs it. A
  timer's `noise` and Lua's `make_noise(place, loudness, faction)` take a
  `faction` too. Any noise still wakes everyone asleep in earshot.

- **Hiding:** a player shut inside something they can't see out of (a
  closed barrel, a wardrobe) can't be seen. Hunters go to where they last
  saw the player, search ("The young wolf searches all around, but finds
  nothing."), and give up. Guards ignore a hidden player.
- **Disguises:** a wearable with `disguise: goblins` makes the goblins
  take the wearer for one of their own. Enemies of goblins now see a
  goblin. Pick fights with goblins and the disguise won't last (see
  faction attitude, above).
- **Bait:** an object with `eaten_by: [wolves]` is food for that faction.
  Left lying in their room, they'll eat it rather than attack, unless
  someone attacks them, and hunters head for food in range. It lasts as
  many bites as its `size`: "The alpha wolf tears into the haunch of
  venison."
- **Bodies tell a story:** examining a body says how it died, from a
  `kills` phrase on the weapon that struck the last blow, the killer, or
  the killer's faction: "The goblin scout has been torn apart by teeth."
  Without one, it names the killer: "You killed the grey wolf." A
  character can start dead, with `state: { dead: true, killed_by: alpha }`.

[The advanced tutorial](docs/ADVANCED.md) builds a whole game around
factions, with all four tricks as ways through.

### Trading

A merchant sells what they carry that has a `price`. The player's money
is `player: { money: 20 }`.

```
> wares
The peddler has for sale: a brass lamp (8 coins) and a rope (30 coins).

> buy lamp
You buy the brass lamp from the peddler for 8 coins.

> i
You're carrying a brass lamp.
You have 12 coins.

> buy rope
The rope costs 30 coins, and you have only 12 coins.

> sell lamp
The peddler offers 4 coins for the brass lamp. SELL it again or ACCEPT to take
that, or ask for more, like "sell the brass lamp for 5".

> accept
You sell the brass lamp to the peddler for 4 coins.
```

- **Prices:** merchants buy at `buys_at` percent of the price (default
  50), and only things with a price. A merchant who haggles names that
  price first, so the player knows there's room to bargain; SELL again,
  or ACCEPT (also DEAL, AGREE), takes it.
- **Merchant money:** with `money:`, a merchant can only pay what they
  have, and gains what the player spends. Without it they can always pay.
- **Looking before buying:** a merchant's goods can be examined before
  they're bought.
- **Haggling:** OFFER 6 FOR LAMP (or BID; HAGGLE FOR LAMP asks how
  much). The asking price or more just buys it. Below that, a merchant
  accepts anything down to `haggle` percent off (default 25; `haggle: 0`
  for fixed prices: "The peddler doesn't haggle. It's 8 coins."). Lower
  offers get a counter ("\"6 coins, and not a coin less,\" says the
  peddler.") and cost a point of the merchant's patience. Out of patience,
  the merchant "looks insulted, and won't trade with you any more" until
  the player apologises.
- **Haggling when selling:** SELL RING FOR 24 (or ASK 24 FOR RING). The
  merchant pays up to `haggle` percent more than its offer, never more
  than its own price for the thing or the money it has: "The peddler
  grumbles, but pays you 24 coins for the gold ring." Ask for more and it
  counters with the most it'll pay ("\"25 coins, and not a coin more,\"
  says the peddler."), costing patience as when buying. ACCEPT takes any
  counter-offer, buying or selling.

  ```
  > offer 2 for the lamp
  "6 coins, and not a coin less," says the peddler.

  > offer 6 coins for the lamp
  The peddler grumbles, but sells you the brass lamp for 6 coins.
  ```
- **Barter:** TRADE RING FOR LAMP (also SWAP, EXCHANGE, BARTER) swaps
  something the player carries for something the merchant sells, with no
  money changing hands. The merchant values the ring at what it would pay
  for it (`buys_at`), and the swap goes through if that's at least the
  least it would take for the lamp (its price, less `haggle` percent): "You
  hand over the silver ring, and the peddler gives you the brass lamp for
  it." If not, "The peddler shakes their head: the silver ring isn't worth
  the rope. Something more, perhaps?", which costs patience like a
  lowball offer (except with a merchant who doesn't haggle). Things without
  a `price` are worth nothing to a merchant.
- **Currency:** prices read "8 coins". To call it something else, reword
  `money.amount` under `messages:`.

### Your own algorithm

Write an `act` script to run on the character's turn instead of its
behaviour. These methods work on any character, in `act` or anywhere
else. Each returns whether it did something, and the game describes it
if the player is there to see:

| Method | Does |
|---|---|
| `self:wander()` | Moves to a random neighbouring room. |
| `self:step_toward(player)` | Moves one room along the shortest way to someone. |
| `self:go("north")` | Takes that exit, if it can. |
| `self:attack(player, weapon)` | Attacks; `weapon` is optional. |
| `self:behave()` | Does whatever its `behavior` would. |
| `self:eat(corpse)` | Eats a body in the same room. |
| `self:is_hostile_to(other)` | Whether its faction would attack them. |
| `self:is_with(player)` | Whether they're in the same room. |
| `self:distance_to(player)` | Rooms apart, by ways it can go, or `nil`. |

```yaml
characters:
  parrot:
    name: parrot
    in: yard
    act: |
      if self:is_with(player) then
        say("The parrot squawks.")
      else
        self:step_toward(player)
      end
  ghost:
    name: ghost
    in: crypt
    behavior: hunter
    act: |
      -- A slow hunter: it only acts every other turn.
      if game.turn % 2 == 0 then self:behave() end
```

`health` and `money` are ordinary numbers to scripts (`player.health =
player.health + 5`), saved and undone with everything else.

## Command reference

| Command | What it does |
|---|---|
| `eifgb check <game.yaml>` | Validates the game and lists errors and warnings, including Lua syntax errors in any script. Exits non-zero on errors. |
| `eifgb play <game.yaml>` | Plays the game in the terminal. |
| `eifgb build <game.yaml> [-o out.json] [--pretty]` | Compiles the game to a story file (JSON), the format players will load. |

Options for `play`:

| Option | Meaning |
|---|---|
| `--commands <file>` | Run commands from a file instead of the keyboard, echoing each one. |
| `--seed <n>` | Fix the random seed. The same seed and commands always give the same game. |
| `--save-file <path>` | Where SAVE and RESTORE keep the saved game. Defaults to `<game id>.sav`. |

### Commands every game understands

Players can type these in any game. Words in the second column mean the
same thing; `…` is where the name of a thing or person goes.

#### Looking and moving

| Command | Also | What it does |
|---|---|---|
| LOOK | L, LOOK AROUND | Describes where you are. |
| EXAMINE … | X …, LOOK AT … | Describes a thing or person. |
| SEARCH … | LOOK IN …, LOOK UNDER …, LOOK BEHIND … | Looks closely, and can turn up hidden things. |
| N, S, E, W, NE, NW, SE, SW, UP, DOWN, IN, OUT | NORTH, GO NORTH, CLIMB UP, … | Goes that way. |
| GO THROUGH … | ENTER … | Goes through a door. |
| GET IN … | SIT ON …, STAND ON …, CLIMB ON …, CLIMB INTO … | Gets into or onto something `enterable`. Standing on something reaches things up high. |
| GET OUT | STAND UP | Gets out again. |
| PEEK NORTH | LOOK NORTH, PEEK THROUGH … | Looks into the next room without going in. |
| EAVESDROP | EAVESDROP NORTH, LISTEN NORTH, LISTEN AT … | Listens through a wall or door. |
| SNEAK NORTH | CREEP NORTH, TIPTOE NORTH | Moves quietly, so fewer characters notice. |
| EXAMINE TRACKS | TRACKS, LOOK FOR TRACKS | Reads the tracks left in the room. |
| FOLLOW … | TRACK … | Follows a creature's tracks: FOLLOW WOLF. |

#### Things

| Command | Also | What it does |
|---|---|---|
| TAKE … | GET …, PICK UP …, TAKE … FROM … | Picks something up. |
| DROP … | PUT DOWN … | Puts down something you carry. |
| PUSH … NORTH | SHOVE, DRAG, PULL, ROLL, SLIDE, MOVE … NORTH | Pushes something heavy into the next room, and follows it. |
| PUT … IN … / PUT … ON … | INSERT … INTO …, PLACE … ON … | Puts a thing into a container or onto a surface. |
| EMPTY … | EMPTY … INTO …, EMPTY … ONTO … | Tips out what's inside. |
| OPEN … / CLOSE … | SHUT … | Opens or closes a door or container. |
| UNLOCK … / LOCK … | … WITH … | Unlocks or locks it, with the right key. |
| WEAR … / TAKE OFF … | PUT ON …, REMOVE … | Puts clothing on or takes it off. |
| EAT … | | Eats something edible. |
| INVENTORY | I | Lists what you carry. |
| LIGHT … | TURN ON … | Lights a lamp, torch or candle. |
| EXTINGUISH … | PUT OUT …, BLOW OUT …, TURN OFF … | Puts a light or a fire out. |
| BURN … | SET … ALIGHT, SET FIRE TO … | Sets fire to something that burns. |
| POUR … ON … | DOUSE … WITH … | Pours water (or something else) on it. |
| SMASH … | BREAK … | Breaks something breakable. |
| THROW … | THROW … AT …, THROW … NORTH | Throws a thing at someone, or into the next room. |
| HIDE … IN … | STUFF … IN … | Hides a thing in a container. |
| BURY … | | Buries a body, so it isn't found. |
| SET … / DISARM … | | Sets or disarms a trap. |
| BLOW … | SOUND …, RING …, PLAY … | Blows a horn, rings a bell. |
| PUSH, PULL, KICK, MOVE, LIFT, SHAKE, TURN, TOUCH, SMELL, LISTEN, TASTE | DRAG, TWIST, FEEL | Tries it out; the game decides if anything happens. |

#### People and creatures

| Command | Also | What it does |
|---|---|---|
| TALK TO … | SPEAK TO …, GREET … | Starts a conversation. |
| GIVE … TO … | HAND … TO …, FEED … TO … | Hands something over: a gift, or food for an animal. |
| BRIBE … | PAY 5 TO …, BRIBE … WITH 5 | Pays someone off. |
| TELL … TO … | ORDER … TO …, ASK … TO … | Gives an order to someone who'll take it: TELL THE DOG TO FETCH THE STICK. |
| CALL … … | NAME … …, RENAME … … | Gives someone a name: CALL THE DOG REX. |
| SHOUT | YELL, SCREAM, MAKE NOISE | Makes a noise that carries. |
| HOWL | BARK, GROWL, IMITATE …, MIMIC … | Mimics a call; only some listeners answer it. |
| BRANDISH … | WAVE …, SHOW … TO … | Shows a weapon (or anything) to scare or impress. |
| APOLOGISE TO … | APOLOGIZE TO …, SAY SORRY TO …, CALM … | Tries to calm someone you've angered. |
| STEAL … FROM … | PICKPOCKET …, ROB … | Takes something without them noticing, or tries to. |

#### Fighting and trading

| Command | Also | What it does |
|---|---|---|
| ATTACK … | HIT …, KILL …, FIGHT …, ATTACK … WITH … | Fights someone. |
| HEALTH | DIAGNOSE | Says how hurt you are. |
| WARES | BROWSE, SHOP | Lists what a shopkeeper sells. |
| BUY … | | Buys something. |
| SELL … | SELL … TO …, SELL … FOR 5 | Sells something, at their price or yours. |
| OFFER 5 FOR … | BID 5 FOR …, HAGGLE FOR … | Makes an offer; they may come back with another. |
| ACCEPT | | Takes their counter-offer. |
| TRADE … FOR … | SWAP … FOR …, BARTER … FOR … | Swaps something you carry for something they sell. |

#### The game itself

| Command | Also | What it does |
|---|---|---|
| WAIT | Z | Lets a turn go by. |
| AGAIN | G | Repeats your last command. |
| UNDO | | Takes back your last turn. |
| SAVE / RESTORE | LOAD | Saves your place, or goes back to a save. |
| RESTART | | Starts the game again. |
| SCORE | | Shows your score, if the game keeps one. |
| HELP | | A short guide to playing. |
| HINT | HINTS, CLUE | The game's next hint for where you're stuck, if it has hints. |
| QUIT | Q | Stops playing. |

Some verbs exist so that players can try them and authors can respond:
PUSH, PULL, KICK, MOVE (DRAG), LIFT, SHAKE, TURN (TWIST), TOUCH (FEEL),
SMELL, LISTEN and TASTE. On their own they give a mild reply ("Nothing
obvious happens."). Use them in `revealed_by`, or with a handler such as
`on: { pull: ... }`. The handler names are the verb IDs `push`, `pull`,
`kick`, `hit`, `move`, `lift`, `shake`, `twist`, `touch`, `smell`,
`listen` and `taste`. TURN's ID is `twist` because `turn` is the
end-of-turn event.

## Cheat sheet

**Top-level sections:** `format`, `game`, `settings`, `presentation`, `vars`, `player`, `rooms`,
`objects`, `characters`, `verbs`, `choices`, `on`, `endings`, `timers`,
`messages`, `scripts`.

**Traits:** `portable`, `scenery`, `container`, `supporter`, `openable`,
`lockable`, `door`, `wearable`, `edible`, `light_source`, `enterable`,
`transparent`, `flammable`, `destructible`, `pushable`. `high: true`
puts a thing out of reach unless the player stands on something. `open` is shorthand for `state: { open: true }`. Things can
also have a `size` (default 1) and a `capacity`; see
[Containers and supporters](#containers-and-supporters). The player's
carrying limit is `player: { capacity: N }`: `0` carries nothing, `none`
(the default) carries anything.

**Characters:** `behavior` (`stationary`, `conversationalist`, `wanderer`,
`merchant`, `hunter`, `guard`, `follower`, `defender`), `wander` (percent),
`area`, `range`, `act` (Lua), `health`, `damage`, `money`, `buys_at`,
`faction`, `corpse`, `fights_back`, `patience`, `pacifist`, `invincible`,
`forgiveness`, `haggle`, `tame_with`, `likes`, `bribe`, `fetches`, `kills`,
`nameable`, `prints`, `leaves_tracks`, `tracker`, `sees_in_dark`, `chatter`, `obeys`, `morale`, `fears`; rooms can be `trackless`,
`diggable` or `dark`; top-level `factions:` with `hostile_to`, `eats_corpses`, `kills`,
`patience`, `prints`, `sees_in_dark`, `morale`, `fears` and `calls`; objects
can have `damage` (weapons), `kills`, `price`, `disguise`, `eaten_by`, `nameable`, `calls`, `trap`, `douses` and (worn) `sees_in_dark`; the player can have `health`,
`damage`, `money`, `voice` and `sees_in_dark`. Events: `defeated` on a character, `died` at the
top level. See [Characters](#characters-npcs-and-enemies).

**Settings:** `describe_state` (default `true`) describes state
automatically in examine text, room descriptions and lists, including
your own fields. See [States and descriptions](#states-and-descriptions).
`list_exits` (default `true`) ends each room description with the ways
out the player can take right now: "Known exits: north and up." Exits
through a secret door that hasn't been found, and exits whose `when` is
false, aren't listed. Turn it off with `settings: { list_exits: false }`,
or for one room with `list_exits: false` on the room (and a room can turn
it back on with `list_exits: true`).

**Presentation:** how your game looks on the website.
`presentation: { theme: terminal, accent: "#c33" }` picks one of the
site's colour themes (`light`, `dark`, `sepia`, `parchment`, `terminal`,
`amber`, `contrast`) and a highlight colour for links and the prompt. Players see
it unless they've picked their own colours with Aa, where it shows as
"The game's own". The terminal player ignores it. The editor sets both
under "How it looks".

**Hidden things:** `state: { hidden: true }` plus `revealed_by: search mat`
(or a list) reveals it when the player does that. SEARCH and verbs with no
handler say "You find a small key." instead of their usual reply. Other
actions add it after: "You open the desk. You find a note."

Instead of one verb, a rule can name an **action category** in brackets.
Case and spacing don't matter:

| Category | Covers |
|---|---|
| `(physical action)` | push, press, shove, pull, tug, yank, kick, hit, punch, strike, smash, break, move, drag, shift, slide, lift, raise, shake, turn, twist, rotate, touch, feel, rub |
| `(inspect)` | examine, look at, search, look under/behind/in |
| `(sense)` | smell, sniff, listen to, taste, lick, touch, feel |

```yaml
  coin:
    state: { hidden: true }
    revealed_by: (physical action) rug    # KICK RUG, DRAG RUG, LIFT RUG... all find it
```

Hide a door to make a secret door. To hide it from one side only, name
that side: a bookcase door with `hidden: library` can't be seen from the
library, but is plain to see from the secret room. Coming through it from
the plain side reveals it on the other side too.

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
`end_game(id)`, `open_choice(id)`, `random(a, b)`, `start_timer(id)`,
`stop_timer(id)`, `timer_left(id)`; on objects,
`:move_to(place)`, `:is_in(holder)`, `:contents()`, `:has(trait)`,
`:can_hold(thing)`, and `.capacity`, which can be changed.

**Custom verbs:**

```yaml
verbs:
  light:
    patterns: ["light {target}", "ignite {target}"]
    default: "That won't burn."
  read:
    patterns: ["read {target}"]
    reach: false        # works on a label behind glass; verbs need to touch things by default
```

Your own verbs win over the standard synonyms: defining READ stops
"read" from meaning EXAMINE.

`{target}` is the thing acted on and `{tool}` is a second thing (`fill {target} with {tool}`).
A verb's ID can't be an event name (`start`, `turn`, `parse_failed`,
`enter`, `leave`), but its patterns can use any words. Defining a verb
with the same ID as one of the try-it verbs above (`kick`, `pull`…) adds
your patterns to it.

## Where to go next

- **[`spec/examples/lighthouse/game.yaml`](spec/examples/lighthouse/game.yaml)**'s
  keeper is a `conversationalist`; see
  [Characters](#characters-npcs-and-enemies) for wanderers, merchants and enemies.
- **[The advanced tutorial](docs/ADVANCED.md)** pits rival factions of
  monsters against each other, and against the player.
- **[The scripting tutorial](docs/SCRIPTING.md)** builds a second game,
  Campfire, step by step. It covers Lua handlers and `block()`, custom
  verbs, script files, `vars`, changing capacity, controlling timers from
  code, `random()` and multiple endings.
- **[`spec/examples/lighthouse/game.yaml`](spec/examples/lighthouse/game.yaml)**
  is a fuller game with comments. It has custom verbs, a dialogue with a
  character, a menu puzzle, global variables, timers for a time limit, a
  scoring system and a Lua helper file.
- **[`spec/FORMAT.md`](spec/FORMAT.md)** is the full reference: every
  section and field, the scripting API and sandbox rules, and the
  **Parser behavior** section, which shows how the game answers typos,
  ambiguous commands and things it doesn't understand.
- **[Sharing your game](docs/SHARING.md)**: a link anyone can play your
  game from, in their browser, and getting it into the gallery.
- **[Writing games with an AI assistant](docs/AI.md)**: the format in
  brief, with a complete checked example, for an AI model or coding agent
  helping you write a game. The website also has
  [`llms.txt`](web/llms.txt) and every docs page as Markdown.

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
  templates/       the editor's starter games (also played to a win by the tests)
```

Run the tests with `cargo test --workspace` and lint with
`cargo clippy --workspace --all-targets`. Every example that has a
`walkthrough.txt` is played to a win by the test suite, so the tutorial
above stays in sync with the engine.

**Feedback from the website** goes to a Google Form: the site posts each
report to the form named in `web/src/feedback-config.ts` (its pre-filled
link, with the answers TYPE, MESSAGE, CONTACT, PAGE, VERSIONS and
CONTEXT), and the answers collect in the form's spreadsheet. With no link
there, the feedback form offers to copy the report instead.
