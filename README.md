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

- [Two ways to make a game](#two-ways-to-make-a-game)
- [Quick start](#quick-start)
- [Tutorial: Midnight Snack](#tutorial-midnight-snack)
- [States and descriptions](#states-and-descriptions)
- [Containers and supporters](#containers-and-supporters)
- [How things behave](#how-things-behave)
- [Pictures](#pictures)
- [Your own words: aliases](#your-own-words-aliases)
- [Timed events](#timed-events)
- [Goals: a journal](#goals-a-journal)
- [Scenes](#scenes)
- [Time of day](#time-of-day)
- [Hints](#hints)
- [Characters: NPCs and enemies](#characters-npcs-and-enemies)
- [Command reference](#command-reference)
- [Cheat sheet](#cheat-sheet)
- [Where to go next](#where-to-go-next)
- [For contributors](#for-contributors)

## Two ways to make a game

**In your browser, with nothing to install:** open
[the editor](https://ravzac14.github.io/EIFGB/editor.html), and start from
a blank game, a starter template, or a copy of an example. Draw rooms on
the map, and fill in what you need in the details panel beside it: each
panel starts with the essentials (a name, a description, where it is),
and everything else is folded underneath, saying what's set. A short
checklist on a new game walks you through to a game that can be won, and
Test play, at the bottom, plays it as you go. Everything in this guide can
be done there; the YAML shown is what the editor writes for you, in the
Files tab.

**With your AI assistant (Claude, say):** on the editor's first page,
*Make a game with your AI* copies the instructions to paste into a chat,
with [the guide for AI assistants](docs/AI.md) in them, so it knows the
format. Describe your story; paste its whole reply back, and it opens as a
game. If the editor finds problems, *Copy for your AI*, under Problems,
gives you a message to send back, and its next reply can be pasted
straight over `game.yaml` in the Files tab: the chat around the code is
left out.

**As files, with the command line:** write `game.yaml` in any editor, and
check and play it with the `eifgb` tool. The quick start below sets it up,
and the tutorial builds a whole game this way.

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

Players can name a door by the way it leads, too: with two doors in a
room, UNLOCK NORTH DOOR picks the right one, and if they just type UNLOCK
DOOR the game asks "Which do you mean, the door (north) or the door
(east)?".

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

A line can also say what the game should answer, after `=>`. The reply
has to contain it (capitals and spacing don't matter), so a change of
wording is caught too:

```
look under the mat => small key
```

A walkthrough for another ending goes in `walkthrough-<ending>.txt`, like
`walkthrough-feast.txt`, and has to reach that ending.

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

Test play also says what the walkthroughs never reach: rooms never
entered, things never seen, and conversation replies never chosen. A
change there isn't checked, so it's worth a playthrough or another
walkthrough. Reach an ending other than the win in Test play, and **Save
for the … ending** saves it as that ending's walkthrough, checked after
every change too.

**Wander** sends a test player through your game: 2000 random moves on
games of its own, without touching yours. It reports script errors, with
the moves that led to each (**Play it here** plays them in Test play),
the endings it reached, how much of the game it got to, and its longest
stretch where nothing new happened, which can be a place players get
stuck.

**Dead ends** looks along the walkthrough for ways to make the game
unwinnable: at each step it tries losing what the rest of the
walkthrough needs (eating it, drinking it, burning it, smashing it,
giving it away, throwing it at someone), and when that's gone for good
and the walkthrough no longer wins, it says so: "After step 1 (TAKE
CAKE), EAT CAKE loses the cake for good, and the walkthrough needs it for
GIVE CAKE TO TROLL." **Play it here** plays it in Test play. That may be
fair, as a loss; if not, give the player another way. It doesn't find
every dead end (a door barred from the wrong side), only those.

**Pacing** follows the walkthrough step by step and draws where things
come: new rooms, new things, points, facts learned and goals done, a dot
for each step that brought one. Stretches of eight steps or more that
bring nothing new are shaded, as places a player might be bored, or
lost. Point at a step to see what it was and what it brought; click it to
play to there. It's a table too.

Test play can also start part-way. **Start at a step…** plays the
walkthrough up to the step you pick without showing it, then lets you
carry on from there. **Start with…** sets the game up as you choose
instead: the room, what the player carries, which doors and lids are open
or unlocked, the variables, and the facts they already know. After a change, it offers to play your moves again on
the new version, so you can try a fix without typing your way back.

Test play also keeps a list of what the game **didn't understand**, under
**Not understood**: a word that isn't a name for anything ("x lantern"),
a verb the game doesn't know ("yoink the gem"), or a word that's only in
a room's description. Each has a fix a click away: add the word as
another name for something in that room, make the verb mean one the game
knows (an [alias](#your-own-words-aliases)), or add it as an object.
Players will try the same things.

The **Puzzles** tab reads the game and shows how each ending comes about:
what has to be found (SEARCH THE MAT: A SMALL KEY), unlocked (THE PANTRY
DOOR WITH THE SMALL KEY: PANTRY OPENS UP) and reached, each after the
steps it needs ("after 2"). It's worked out as a graph, so puzzles that
don't depend on each other stay apart: each ending lists only the steps
it takes, and **side puzzles** are the ones no ending needs. A step that
can be done more than one way says so ("or: unlock the desk with the code
4512"). A way that waits on a variable (`when: vars.bridge_down`) says
what sets it ("once PUSH THE WINCH sets vars.bridge_down"), and one that
waits on a value (`when: vars.side == "rebels"`) needs what sets that
value.

**Either-or choices.** When only one of several things can happen, the
chain treats them as a choice: options or handlers that each check a
variable isn't set yet and then set it. For example, "Take the sword" and
"Take the shield", each with `when: not vars.armed` and `do: 'vars.armed =
true; …'`. Each shows as a step saying what it rules out. An ending that
depends on one says "Only if the player chooses “Take the sword”". An
ending that needs two of them can't happen, and a choice that leaves no
way to win is listed under getting stuck.

Above that, it lists what can never happen: a room nobody can get to, a
key that opens nothing, something locked with no key, a hidden thing
nothing reveals, an ending nothing leads to, a way waiting on a variable
nothing sets. And **where a player could get stuck**: something they need
to win that can be eaten, burnt or smashed, that a script can do away
with (`move_to(nil)`), a key that wears out (`uses`) before the last lock
it's needed for, or a choice that rules out the only way to win. It
follows exits, doors, keys, codes, containers, `revealed_by`,
conversations, timers and the Lua that names things (`world.cellar`,
`end_game("won")`). Click a line to go to it.

**Steps solved in scripts.** The chain reads your game; it doesn't play
it, and it can't see inside Lua. It says so rather than guess:

- A step that leans on a script says **solved in a script**, with a link
  to it. Check that the script does what the step says.
- **What the chain takes on trust** lists every assumption on the way to
  an ending: scripts it can't see inside, conditions it can't read
  (`when: clock.night` is assumed to come true), and things it assumes
  the player can get from someone ("by asking, buying, stealing or
  fighting").
- Some scripts it can read. A handler for a verb is read as that action.
  If it checks what it was done with (`args.tool == world.child`), the step
  needs that too. So `after_give` on the bread, with that check, is "Give
  the loaf of bread to the hungry child". A merchant's priced goods are
  bought. Writing handlers this way keeps the chain accurate.

The walkthrough in Test play is the check for the rest: it plays the game
for real after every change.

**Draft hints from these steps** turns the chain into [hints](#hints) for
HINT: for each thing to find and door to unlock, a nudge, a clue and the
answer, shown until the player has done it (`when: not
world.small_key.seen`, or `when: world.pantry_door.locked`). The steps on
the way to an ending come first, so HINT helps with the earliest one still
to do. They're drafts: reword them in your own voice.

The **Timers** tab shows when each [timer](#timed-events) can go off: a
mark after so many actions, a shaded span for a random wait (`after:
5-10`), every time a repeating one goes off, and, with a
[clock](#time-of-day), the time of day that would be. It says what each
one does and what can stop it. A dashed line means the timer counts from
when something starts it, not from the start of the game.

A few more tools in the editor:

- **The details panel** starts with what most things need (a name, a
  description, where it is), and folds everything else under headings
  that say what's set in them ("What they do · Hunts the player"). Open
  one to change it; it stays open while you work. On a new game, a
  **Getting started** checklist ticks off the steps to a game that can be
  won. *Don't show this again* hides it for every game in this browser;
  *Show the Getting started checklist again*, under Drawing the map,
  brings it back.
- **Text that varies:** under a description, *Say something different
  each time…* gives it a box for each line, and a choice of each in turn,
  the first time and then another, round and round, or at random.
- **Routines and scenes:** a character's *What they do* has a daily
  routine (a time and a place for each row), and the game's panel lists
  scenes beside goals, each with when it begins and ends.

- **Find** (Ctrl+Shift+F) searches every room, thing, character,
  conversation, message and script for a word, a name or an ID. Click a
  result to go to it.
- **History**, beside Undo and Redo, lists your recent changes, newest
  first, each saying what it was ("Added the room Attic"). Click one to go
  back to before it; going back can be redone, until you change something
  else.
- **Tidy up**, on the map, lays your rooms out again by their directions
  from the start room. Ways that are hard to read on the map are drawn
  dashed in red, and listed: two lines that cross, a line running through
  another room, and an exit drawn going the opposite way to its direction.
  Only the map changes, never the game.
- **Conversations as a graph:** select a conversation and the map shows
  its parts instead, each with its replies, and a line from each reply to
  where it leads. Drag a part to move it; drag from a reply's dot onto a
  part to make it lead there, or into empty space for a new part. Click a
  part to edit it.
- **Words in descriptions:** under a description, the words players can
  already use for something here are highlighted, and things it mentions
  that aren't anything yet are underlined, each a click from being
  scenery ("A narrow path climbs north" means someone will type EXAMINE
  PATH).
- **Read it aloud**, under a description, speaks it in the browser's
  voice: a sentence that runs on, or a word said twice, stands out when
  it's heard.
- **What a player might try**, on a room's panel: for each thing and
  person there, what a curious player types (PULL LEVER, SMELL ROSE, TALK
  TO BAKER), split into what the game answers with something of its own
  (a script, a smell, a description) and what gets only the usual reply.
  A lever that does nothing when pulled is worth a second look.
- **The tricks, in the panels:** a thing's panel has Far off, and a
  Tricks section (reaches, a mirror, what it comes apart into, a mood it
  brings on when eaten or drunk, what it combines with and makes, leaves
  a mark, a fake of something, puts out fires, a trap, a disguise, bait);
  a character's has what they watch and what wins them over (gifts they
  like, taming, a bribe), A favour they ask, Playing as them, Voices and
  friends, and What they remember. The game's panel lists what the player
  can learn (its facts), and a conversation's replies say what they teach
  and whether they promise a favour. Moods are named in the
  game's panel, and then each room, thing and person can say how it reads
  in each one.
- **How characters sound**, under Problems: what each character says
  (in conversations, chatter, favours and memories, the words in quotes),
  side by side, with how long their sentences run, how often they ask or
  exclaim, how often they say "don't" for "do not", and words only they
  use. Two characters who sound alike, or one whose later lines sound
  unlike their early ones, are pointed out, as guesses to read again.
- **How the descriptions read**, under Problems: the room, thing and
  character descriptions read together. Words leaned on in description
  after description ("ancient", 14 times), descriptions longer than 120
  words, ones that are hard going (a reading grade of 12 or more: long
  sentences, long words), and rooms that all begin the same way ("You are
  standing…"). Only pointers, to read and decide.
- **Kinds**: in a thing's, character's or room's details, **Make a kind
  of this** turns its fields into a kind others can be like; **Its kind**
  ticks which kinds something is like. The game's panel lists the kinds
  and what's like each. See [Kinds](#kinds).
- **Possible typos**, under Problems: words that aren't in the dictionary
  or the game's names, a letter off a word that is ("lantren", perhaps
  "lantern"), however often they're used. The dictionary is a word list
  kept with the site ([SCOWL](http://wordlist.aspell.net/), US and British
  spellings), fetched once: nothing of the game is sent anywhere. Until
  it's loaded (or offline), the check is the game against itself: words
  used once that are a letter off one it uses elsewhere. The game's own
  words are always allowed, and the browser spell checks text boxes too.
- **Pictures** can be shared: a room, thing or character can use a
  picture that's already in the game, which is kept once. Under each
  picture, the editor says how much space the game's pictures take, and
  how full the browser's storage is.

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

### Text that varies

Anything said often can say something different each time. A description,
a verb's `default`, a timer's `say`, what a conversation part or reply
says, and any message can be a list, or one of these:

```yaml
rooms:
  square:
    name: Square
    description:
      first: You step into the square for the first time. It's busy.
      later: The square again${if here.visits > 2}, as familiar as your hand${end}.
  bakery:
    name: Bakery
    # A list: each in turn, then the last one from then on.
    description: [Warm and floury., Still warm., Warm.]

messages:
  # Round and round.
  wait.done: { cycle: [Time passes., A cart rumbles by.] }
  # At random, never the same twice in a row.
  listen.nothing: { random: [All quiet., A dog barks somewhere., Nothing.] }
```

Each one remembers how often it's been shown, with the saved game, so
UNDO and RESTORE take that back too. Random ones use the game's own
random numbers, so a walkthrough plays the same every time. For a room,
`here.visits` (or `world.square.visits`) counts the times the player has
come in: 1 on the first visit.

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

### Stairs, paths and bridges

Players often say how they go rather than which way: CLIMB THE STAIRS, GO
DOWN THE PATH, CROSS THE BRIDGE. Give the exit a `via`, naming the thing
it goes by (or a list of them):

```yaml
rooms:
  hall:
    name: Front Hall
    exits:
      up: { to: attic, via: stairs }
  attic:
    name: Attic
    exits:
      down: { to: hall, via: stairs }
objects:
  stairs: { name: staircase, synonyms: [stairs, steps], in: hall, traits: [scenery] }
```

Now CLIMB THE STAIRS, GO UP THE STAIRS, TAKE THE STAIRS and USE THE STAIRS
go up, and from the attic, GO DOWN THE STAIRS comes back. Like a door, the
staircase is there on both sides of every way that goes by it, even though
it's only `in:` the hall. In the editor, select the way out on the map and
use **Goes by**.

## How things behave

A few fields give things the behaviour players expect, with no scripts.

```yaml
rooms:
  camp:
    name: Camp
    outdoors: true                    # the weather shows here
    sound: The stream babbles.        # LISTEN here, and from next door
    smell: Woodsmoke drifts past.
objects:
  candle: { name: candle, in: camp, traits: [portable, light_source], needs_flame: true, fuel: 20 }
  matches:
    name: box of matches
    in: camp
    traits: [portable, light_source]
    fuel: 2                           # burns for 2 turns…
    burns_away: true                  # …then it's gone
    spent: The match burns down to your fingers.
  bottle: { name: glass bottle, in: camp, traits: [portable], holds_liquid: true }
  stream: { name: stream, in: camp, traits: [scenery], liquid: water }
  rope: { name: old rope, in: camp, traits: [portable], uses: 3, breaks: The rope frays and snaps. }
  safe: { name: safe, in: shed, traits: [container, openable, lockable], state: { locked: true }, code: 4512 }
  note: { name: note, in: shed, traits: [portable], text: '"The code is 4512."' }
```

- **Flames:** a light that `needs_flame` (a candle, a torch) lights only
  from something burning: LIGHT THE CANDLE WITH THE MATCHES (a lighter or
  match you're holding is struck first), or with any flame in hand. STRIKE
  and FLICK light things too.
- **Fuel:** a light with `fuel` burns for that many turns, flickers two
  turns before the end ("It won't last much longer."), then goes out, or
  is gone if it `burns_away`. Its `fuel` state counts down, so a script can
  refill it: `world.lamp.fuel = 20`.
- **Liquids:** something that `holds_liquid` (start it full with
  `full_of: water`) can be filled from anything with a `liquid`: FILL THE
  BOTTLE FROM THE STREAM. Then POUR OUT THE WATER, POUR THE BOTTLE INTO THE
  BUCKET, DRINK THE WATER, or POUR THE WATER ON THE FIRE: water puts out
  fires and flames, and so does any liquid whose source `douses`. "The
  water" means the bottle with water in it. Its `liquid` state says what's
  in it; lists say "a glass bottle (full of water)".
- **Wear:** something with `uses` breaks after being used that many
  times as a tool (a key, a knife, a rope), or by one of your verbs'
  handlers. `breaks` says how. On its last use it says so ("The iron key
  won't last much longer."), and examining it says it's nearly worn
  through. `breaks_into: broken_key` leaves something behind (start it
  with no `in:`); see [Mending](#ropes-knocking-and-mending).
- **Codes:** something lockable with a `code` unlocks when the player
  types it: TYPE 4512, ENTER 4512 ON THE KEYPAD, OPEN THE SAFE WITH 4512,
  DIAL FOUR SEVEN ONE, TURN THE DIAL TO 4-7-1. Hide the code in a clue,
  or a [picture](#pictures). `tries: 3` jams it after three wrong codes
  (it beeps, and anyone hunting nearby comes to look); `feedback: true`
  says how close a wrong one was: "2 in the right place, and 1 more right
  but in the wrong place." Its `tries` state counts down, so a script can
  reset it.
- **Writing:** `text` is what READ shows (a book, a note, a sign), apart
  from what EXAMINE says about the thing itself.
- **Sounds and smells:** LISTEN and SMELL say a room's `sound` and
  `smell`, those of what's in it, and those of the rooms next door ("From
  the north: Bats squeak."). LISTEN TO THE CLOCK gives the clock's.
- **Companions in the dark:** a character carrying a lit lamp lights the
  room they're in. Companions (who follow the player, or obey them) are
  known even in the dark, and one that `sees_in_dark` can be sent for
  what the player can't see: REX, FETCH THE BONE.

### Weather

```yaml
weather:
  start: clear
  changes: 20-40          # turns; a range picks at random. Leave it out: scripts only.
  kinds:
    clear: { describe: The sky is clear., begins: The clouds blow away. }
    rain:  { describe: Rain patters on the leaves., begins: It starts to rain. }
```

Rooms that are `outdoors` add the weather's `describe` to their
description, and a player outdoors is told its `begins` when it changes.
Scripts read `game.weather` and change it with `set_weather("rain")`, and
descriptions can depend on it: `${if game.weather == "rain"}The path is
muddy.${end}`. In the editor: **Weather** in the game's details.

### Water

A room can flood:

```yaml
player: { breath: 4 }       # turns the player can hold their breath (4 unless set)

rooms:
  cellar:
    name: Cellar
    floods: { every: 3, depth: 3, drains: sluice }
objects:
  sluice: { name: sluice gate, in: cellar, traits: [openable] }
  barrel: { name: empty barrel, in: yard, traits: [portable], floats: true }
```

The water rises a level every `every` turns (3 unless set), up to
`depth`: 1 ankle-deep, 2 waist-deep, 3 over the player's head (the
default). LOOK says how deep it is, and the player is told as it rises.
Over their head, they swim (SWIM NORTH), and hold their breath: when it
runs out, they struggle back the way they came, and lose a point of
health if they have any. Something that `floats`, carried, keeps them
up. Opening the `drains` thing lets the water out, a level a turn.
`starts: false` waits for a script: `flood(world.cellar)` starts it
rising, `flood(world.cellar, 2)` sets it waist-deep, `flood(world.cellar,
0)` drains it. Scripts read `world.cellar.water` (0 to 3). SWIM is only
a command in games with water, so a game's own answer to it stands.

### Weight

```yaml
objects:
  plate: { name: stone plate, in: vault, traits: [supporter], size: 4, pressure: { at_least: 3, opens: gate } }
  raft: { name: raft, in: river, traits: [supporter, enterable], capacity: 2, sinks: true }
```

A `pressure` plate opens its doors (or lids) while there's at least that
much weight on it, and shuts them again when there isn't: a boulder, or
the player standing on it with enough in their pack. A thing weighs its
`size` and what's inside it; the player, their own `size` (1 unless set)
and what they carry. `down:` and `up:` reword "The stone plate sinks
under the weight, with a click." Something that `sinks` says it would
sink, instead of that there's no room, when too much is put on it or the
player is too heavy to get on.

### Scents

A character that `sniffs` (a dog) can be given a scent: LET THE DOG
SNIFF THE GLOVE. It leads the way, a room at a time, waiting for the
player to catch up, to whoever the glove belongs to: its `scent_of:` (a
character or thing), or whoever had it at the start. There it barks; a
hidden thing, it digs up.

### Echoes and whispers

```yaml
rooms:
  cave:
    echo: { help: '"…help… help…" And then, faintly, from far below: "Down here!"' }
    hears: [tunnel]
```

SHOUT or SAY anything in a room with an `echo` (`true`, or answers to
words) and it comes back: "…hello… hello…", or the answer for a word
said. A room that `hears` other rooms is a whispering gallery: what
characters say there is heard here, word for word.

### Ropes, knocking and mending

```yaml
rooms:
  cliff: { name: Cliff Top, exits: { down: { to: ledge, rope: true } } }
objects:
  rope: { name: rope, in: shed, traits: [portable], rope: true }
  post: { name: post, in: cliff, traits: [scenery], anchor: true }
  panel: { name: loose panel, in: study, traits: [portable], hollow: true, state: { hidden: true } }
  key: { name: iron key, in: hall, traits: [portable], uses: 3, breaks_into: broken_key }
  broken_key: { name: broken key, traits: [portable] }
  glue: { name: pot of glue, in: shed, traits: [portable], mends: true }
  bread: { name: loaf of bread, in: kitchen, traits: [portable, edible], spoils: 40 }
characters:
  porter: { name: porter, in: gate, password: { word: swordfish, bars: north } }
```

- **Ropes:** TIE THE ROPE TO THE POST (anything that's an `anchor`).
  An exit with `rope: true` is a sheer drop, only to be climbed with a
  rope tied at the top or the bottom: GO DOWN, or CLIMB THE ROPE, from
  either end. A tied rope can't be taken until it's untied (UNTIE THE
  ROPE). TIE UP THE GUARD (WITH THE ROPE) works on someone asleep,
  caught in a trap, cowering or nearly beaten: tied up, they do nothing
  at all until someone unties them, though they can still be talked to.
  Scripts read `world.rope.tied_to` and `world.guard.bound`.
- **Knocking:** KNOCK ON THE WALLS finds anything `hollow` that's hidden
  in the room; KNOCK ON THE WARDROBE, anything hollow hidden in it. KNOCK
  ON A DOOR, and whoever's on the other side may open it: if they don't
  dislike the player, and it isn't locked (or they have its key).
  Someone hostile comes to see who it is.
- **Passwords:** a character with a `password` won't let the player go
  the ways it `bars` (The porter steps in your way. "Password?"),
  until they SAY it. `ask:`, `right:` and `wrong:` reword it. Asleep or
  tied up, they let anyone by.
- **Mending:** MEND THE KEY (WITH THE GLUE), with something that
  `mends`: something worn is as good as new, and what a thing
  `breaks_into` is made whole again. Give the mender `uses` to have it
  used up.
- **Going bad:** food or drink that `spoils` goes bad that many turns
  into the game, wherever it is ("The loaf of bread has gone bad."), and
  won't be eaten. Its `spoiled` state turns true.

### Boats, horses and crowds

```yaml
rooms:
  quay: { name: Quay, exits: { east: { to: river, by: boat }, north: { to: road, by: horse } } }
  market: { name: Market Square, crowd: "8:00-18:00" }     # or crowd: true
objects:
  boat: { name: rowing boat, in: quay, traits: [container, enterable], vehicle: true }
characters:
  horse: { name: horse, in: stable, mount: true, stamina: 12, likes: [apple], disposition: friendly }
```

- **Boats and carts:** something `enterable` that's a `vehicle` carries
  the player along the ways that are `by` it: GET IN THE BOAT, then ROW
  EAST (or GO EAST, SAIL, PADDLE…). Any other way, they get out and walk.
  On foot, a way `by` the boat says "You can't go that way without a
  rowing boat."
- **Horses:** RIDE THE HORSE, if it's tame, friendly, or obeys the
  player: it goes where they go, ways `by` it are open, and GO TO takes
  half the turns. With `stamina`, it tires after that many moves, and
  won't go on until it's fed something it `likes`. DISMOUNT (GET OFF) to
  get down. Scripts read `world.player.riding`.
- **Crowds:** a `crowd` room (always, or at the hours given, in a game
  with a clock) says so when the player looks. Nobody who'd go for the
  player can pick them out of it, unless they make a scene (attack
  someone, or get caught stealing); thieves are hardly ever noticed by
  their victims, though someone in the crowd may shout "Thief!"; and
  nobody leaves tracks there. Scripts read `world.market.crowded`.

### Hands and the cold

```yaml
player: { hands: 2, warmth: 8 }
rooms:
  pass: { name: Mountain Pass, cold: true }
objects:
  chest: { name: chest, in: hall, traits: [portable, container], two_handed: true }
  coat: { name: fur coat, in: hall, traits: [portable, wearable], warm: true }
  soup: { name: bowl of soup, in: inn, traits: [portable, edible], warm: true }
```

- **Hands:** with `hands:` on the player, only so much can be held. Big
  things (size 2 or more), weapons and burning lights take a hand, and
  `two_handed` things both; small things go in pockets, and worn things
  don't count. "Your hands are full: you're holding the torch and the
  shield." Climbing (a rope, or CLIMB) takes a hand free. INVENTORY says
  what's in their hands.
- **The cold:** in a `cold` room, or out of doors in weather that's
  `cold: true`, the player lasts `warmth` turns (8 unless set), shivering
  near the end; then each turn costs a point of health, or, if they
  can't be hurt, they freeze (the `froze` ending, which you can write
  yourself). Something `warm` worn keeps the cold out; a fire in the room
  (something on fire, or a fixed flame like a hearth) warms them quickly;
  anywhere else, slowly. Eating or drinking something `warm` warms them
  right through. Soaked by deep water, the cold bites twice as hard. The
  `warmth` state counts down, for scripts to read or set.

## Pictures

A room, an object or a character can have a `picture`. A room's is shown
under its name, above its description (and with the map, while the
player is there). A thing's is shown when the player examines it, which
makes pictures good for clues the player has to see for themselves: a
photograph, a map, a painting, a note.

```yaml
rooms:
  hall:
    name: Front Hall
    picture: { file: pictures/hall.jpg, alt: "A long hall, its wallpaper peeling." }
objects:
  note:
    name: note
    in: kitchen
    traits: [scenery]
    picture:
      file: pictures/fridge_note.svg
      alt: 'A sticky note: "Keys live on the hook. NOT down the back of the sofa."'
```

- **`file`** is a picture beside `game.yaml` (a `.png`, `.jpg`, `.gif`,
  `.webp` or `.svg`), or an `https://` address. `picture: pictures/hall.jpg`
  on its own works too.
- **`alt`** says what the picture shows, for players who can't see it:
  screen readers, read aloud, and the terminal, which prints
  `[Picture: …]`. If the picture holds a clue, put the clue in `alt` too.
  The compiler warns when it's missing.
- A thing with a picture and no `description` shows just the picture.
- In the editor, every room, object and character has a **Picture**
  section: choose a file (it's made smaller, to fit in the browser, and
  kept with the game) or paste a web address. **Add component → Picture**
  adds something to look at. Downloads, single web pages and links to a
  game's folder all bring their pictures along.

## Your own words: aliases

Players type the words they know. If your game's players say "using"
where the game expects "with", or a sailing game wants PORT for west, add
an `aliases` section. Each word on the left means the words on the right,
wherever it appears in a command:

```yaml
aliases:
  using: with          # UNLOCK DOOR USING KEY
  port: west           # PORT, GO PORT
  starboard: east
  yoink: take          # YOINK THE GEM
  grab hold of: take
```

An alias stands for words the game already knows: a verb, a direction, a
thing's name, or a word like "all". The compiler warns when the right-hand
side isn't a word the game knows, and when the left-hand side already
means something (the alias then replaces it everywhere). For new verbs
with their own behaviour, add a verb under `verbs:` instead. In the
editor, aliases are under **Words** in the game's details.

## Kinds

Something you'd otherwise write out again and again (guards, lamps, a
stretch of forest) can be written once, as a kind, and the rest can be
`like` it:

```yaml
kinds:
  guard: { name: guard, behavior: guard, faction: watch, health: 5, damage: 2, state: { alert: true } }
  lamp: { traits: [portable, light_source], needs_flame: true, fuel: 20 }
  forest: { outdoors: true, sound: Leaves rustle overhead., trackless: true }
rooms:
  wood_1: { like: forest, name: Dark Wood, exits: { north: wood_2 } }
  wood_2: { like: forest, name: Deeper Wood, exits: { south: wood_1 } }
objects:
  candle: { like: lamp, name: candle, in: hall, fuel: 10 }
characters:
  north_guard: { like: guard, in: gate }
  captain: { like: guard, name: captain, in: keep, health: 9 }
```

Something that's `like` a kind starts with the kind's fields, and its
own go on top: the captain's `health: 9` replaces the guard's 5. Maps
(`state`, `on`, `moods`…) are merged, and `traits` add up, so a kind's
traits and the thing's own are all there. A kind can be `like` another,
and something can be like several (`like: [guard, archer]`, the later on
top). The compiler fills them in before anything else, so everything
else reads the game as if it were written out in full.

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
| `after: N` / `every: N` / `at: time` | Fire once N actions after starting, every N actions, or each day the clock reaches a time (`"19:00"`, `"7:30 pm"`, `dawn` or `dusk`; needs a [clock](#time-of-day)). Exactly one is required. N can be a range, like `every: 5-10`: a random number of actions in it, picked again each time, so the player can't count on it. |
| `started_by`, `stopped_by` | Rules like `revealed_by`: `"<verb> <object>"`, `"(<category>) <object>"`, or a list. Starting a timer that's already running doesn't reset it. |
| `running` | Whether it counts from the start of the game. Defaults to `true`, or `false` if it has `started_by`. |
| `when` | A Lua condition checked when it's due. If false, this firing is skipped. |
| `say`, `do`, `end` | What happens: print text, run Lua, end the game. Any combination. |
| `say_again` | Said instead of `say` every time after the first: `say: Thunder rolls in the distance.`, then `say_again: More thunder.` Leave it out to say the same each time. |
| `chance` | The chance it goes off each time it's due: `chance: 10%`. With `every: 1`, something that might happen any turn. |
| `heard_in` | Where `say` is heard: a room, a list of rooms, or a thing (a clock, heard wherever it is). Elsewhere the timer still goes off (its `do` and `end` happen), just unannounced. `say_again` starts after the first time it's heard. |
| `noise: { at, loudness, faction }` | A noise that hunters within `loudness` rooms (of `faction` only, if given) come to investigate. `at` is a room, or a thing (the noise is wherever it is); without it, wherever the player is. See [Tricks players can find](#tricks-players-can-find). |

A timer started by an action counts the actions *after* that one. With
`after: 3`, lighting the fuse and then waiting three times gives the
explosion on the third wait. Timers are saved and undone with everything
else.

Scripts can also use `start_timer("bell")`, `stop_timer("bell")` and
`timer_left("bell")`. `timer_left` gives the number of actions left,
counting the current one, or `nil` if the timer isn't running.

## Goals: a journal

Goals tell players what they're trying to do. JOURNAL (or GOALS) lists
the ones they have, and a notice says when one is added or done:

```yaml
goals:
  find_keys:
    text: Find your keys.
    done: world.keys.location == player     # done once this is true
  catch_bus:
    text: Catch the 9:15 bus.
    when: here == world.front_garden        # added once this is true
    done: vars.on_bus
    points: 5                               # scored when it's done
```

| Field | Meaning |
|---|---|
| `text` | What JOURNAL lists. Text, so `${…}` works. |
| `when` | A Lua condition: the goal is added once it's true, and stays. Without it, the goal is there from the start. |
| `done` | A Lua condition: the goal is done once it's true, and stays done. Without it, only a script can finish it. |
| `points` | Added to the score when it's done. |

"[New goal: Find your keys.]" and "[Done: Find your keys.]" appear after
the action that did it; the first goal also says that JOURNAL lists them.
JOURNAL shows the goals still to do, in the order they're written, then
those done. It takes no turn. Goals are saved and undone with everything
else.

Scripts can use `complete_goal("catch_bus")` and `start_goal("catch_bus")`
(which adds it whatever its `when`), and read `goals.catch_bus`: `nil`
before it's added, then `"active"`, then `"done"`.

In the web editor, add one with **Add component → Goal**, or under
**Goals** in the game's details.

Players can also keep notes of their own, in any game: NOTE THE GUARD
SLEEPS AFTER DARK. A note is the rest of the line exactly as typed, it
takes no turn, and JOURNAL (or NOTES) lists them under the goals. Notes
are kept with the game's saves, but UNDO and RESTART leave them alone:
they're the player's, not the story's.

## Scenes

A scene is a named stretch of the story: the morning rush, the storm, the
chase. It begins when its `when` comes true (without one, at the start),
and ends when its `done` does:

```yaml
scenes:
  storm:
    title: The storm
    when: vars.clouds_gathered
    done: clock.hour >= 18
    begins: The first drops of rain hit the cobbles.
    ends: The storm blows itself out.
```

`begins` and `ends` are said then, if given. While a scene is going on,
RECAP says so ("Now: The storm."), and conditions can check it:
`scenes.storm` is `nil` (not begun), `"running"` or `"done"`. Scripts can
begin and end scenes themselves with `start_scene("storm")` and
`end_scene("storm")`.

## Flashbacks

A flashback is a memory the player plays through. What they carry is set
aside, and they're somewhere in the past, with what the memory gives
them, until it's over; then they're back where they were, with what they
had.

```yaml
flashbacks:
  childhood:
    at: garden_then              # a room of the past, reached no other way
    carries: [ring_then]
    turns: 6                     # or until: 'world.ring_then.location == world.garden_then'
    begins: You're small again, and the garden is enormous.
    ends: The memory fades. You're on the quay, holding the locket.
rooms:
  garden_then: { name: "The Garden, Long Ago", now: garden }
objects:
  locket: { name: locket, in: quay, traits: [portable], flashback: childhood }
  ring_then: { name: little ring, traits: [portable], now: ring }
  ring: { name: old ring, traits: [portable] }   # no `in:`: the past decides
```

EXAMINE or READ the locket (the first time) and the memory plays; or a
script calls `flashback("childhood")`, and `end_flashback()` ends one
early (`in_flashback()` says which is playing). What the past does stays
done: a past thing with `now:` left in a past room with `now:` is found
in the present room afterwards. Drop the little ring in the garden then,
and the old ring is in the garden now. Variables set in the past stay
set.

## Epilogue

After an ending, the epilogue looks back on what the player did:

```yaml
epilogue:
  - { when: 'world.dog.name == "Rex"', text: Rex lived to be very old. }
  - { when: 'world.baker.promise == "broken"', text: The baker never forgave you. }
  - { endings: [escaped], text: You never went back to the island. }
  - And the lighthouse still shines.
```

Each line is said after the ending's own text, if its `when` holds (and
only after the `endings` it names, if any). A line on its own is always
said.

## Moods

The player can be drunk, frightened, dreaming, or anything else a game
needs, and the world looks different while they are. It works the way
darkness does: in a dark room, the room's description gives way to "It's
pitch dark", and in a mood, a room's, thing's or character's `moods:` text
for it takes the place of its description.

```yaml
moods:
  drunk: Everything sways, just a little.

rooms:
  hall:
    name: Hall
    description: A plain hall.
    moods: { dreaming: The hall stretches on forever. }

objects:
  wine: { name: cup of wine, in: hall, traits: [portable, drinkable], mood: { drunk: 5 } }
```

A mood is a player state of that name: `player.state.drunk = true` in a
script turns it on, `nil` off. Something eaten or drunk with `mood: {
drunk: 5 }` brings it on for that many turns (0: until a script ends it),
and "[You feel more yourself again.]" when it passes. The top-level
`moods:` adds its line to LOOK in every room without its own text for
the mood.

## Several player characters

A game can let the player be more than one person: a heist crew, a
family. Mark each with `playable: true`, and say who the player starts
as:

```yaml
player: { plays: mara }

characters:
  mara: { name: Mara, in: hall, playable: true, description: A wiry thief. }
  tobin: { name: Tobin, in: kitchen, playable: true, description: A big cook. }
```

SWITCH TO TOBIN (BECOME, PLAY AS) makes the player Tobin, where he is,
with what he carries and his health; Mara stays where she was, holding
what she held. EXAMINE ME describes whoever the player is now. What the
player has seen and learned is shared between them. Characters who are
enemies of one are enemies of all, since it's the player they know.

## What the player learns

A game can name the things worth knowing, as `facts:`, and say where
the player learns each: in a conversation, or reading or looking at
something.

```yaml
facts:
  meeting: The smugglers meet at midnight.
  mill: { text: The smugglers meet at the old mill, past the bridge., place: mill }

objects:
  plaque: { name: plaque, in: bridge, text: Built in 1802., teaches: built }

choices:
  gossip:
    start: hello
    nodes:
      hello:
        options:
          - { text: Any news?, say: '"They meet at midnight."', teaches: meeting }
          - { text: Where?, when: 'knows("meeting")', say: '"The old mill."', teaches: mill }
```

`teaches:` (one fact, or a list) goes on a conversation's node or option,
or on a thing: READ teaches it (EXAMINE, if there's nothing written on
it). The first time, the player sees "[You'll remember that.]". Asked
again, the one telling them says "As I told you before…". WHAT DO I KNOW
(WHAT HAVE I LEARNED, CLUES, RECALL) lists everything, and who said it;
WHAT DO I KNOW ABOUT THE BRIDGE just what mentions the bridge. Scripts
check `knows("mill")`, as the option above does, and `learn("mill")`
teaches it.

### Places heard of

A fact with a `place:` tells of a room. Until the player has been there
it's on their map with a "?", and MAP lists it as heard of. GO TO THE
MILL needs the way: ASK THE GOSSIP THE WAY TO THE MILL (WHICH WAY TO …,
HOW DO I GET TO …) gets "North twice, then east", and after that GO TO
walks there. People you can talk to give directions (and shopkeepers);
`knows_the_way: true` or `false` on a character says otherwise. A room
with `heard_of: true` is known from the start (the castle on the hill),
and a script can `hear_of(world.mill)`.

### Promises

A character can ask a favour: bring them something.

```yaml
characters:
  baker:
    name: baker
    in: shop
    asks:
      for: cat
      say: '"Have you seen my cat? Would you find her?"'
      promise: find her cat
      thanks: '"Whiskers!" The baker hugs the cat.'
      gives: coin
      within: 40
      broken: '"You said you''d find her," says the baker coldly.'
```

TALK TO THE BAKER (if she has no conversation) says what she asks.
PROMISE (I'LL DO IT, PROMISE THE BAKER) promises it, and she warms to the
player; JOURNAL lists promises. Giving her the cat keeps it: her
`thanks`, and what she `gives`. Asked or not, she's grateful, but more so
for a promise kept. Past `within` turns, the promise is broken: she
cools, and her friends hear of it, and next time she says her `broken`
line. An option with `promise: true` in a conversation promises too, as
does `promise(world.baker)` in a script, and scripts read
`world.baker.promise`: nil, "made", "kept" or "broken".

### Copying

COPY THE PASS ONTO THE PAPER makes a forgery: the pass's `copy:` (an
object that starts with no `in:`), from something `blank: true` (paper,
used up) and something that `writes: true` (a pen) both carried. A copy
works as a fake of the original (as with `decoy_for`), so it can be
swapped for it unseen. COPY THE INSCRIPTION, of something with `text`
and no `copy:`, writes the words into the player's notes.

### Notes

With the same paper and pen, WRITE "MEET ME AT THE MILL" ON THE PAPER.
READ shows what the player wrote, and scripts read `world.paper.written`.
Give the note to someone who `reads` (characters with something to say
do, unless `reads: false`), or leave it where they'll find it. If they
believe it (they don't dislike the player, or it names one of their
`friends` or someone they answer to) and it names a place ("the mill",
for The Old Mill), they go there and wait a while: long enough to slip
past, or to take what they were watching. A character's `read_note`
handler (`on: { read_note: ... }`, with `args.text`) answers instead.

## Time of day

A game can keep a clock that moves on with each turn:

```yaml
clock:
  start: "8:50"            # when the story starts: "8:50", "19:30", "7:30 pm", "noon"
  minutes_per_turn: 1      # how far each action moves it (default 1)
  dawn: "6:00"             # night is from dusk until dawn (these are the defaults)
  dusk: "18:00"
  show: true               # the time on the status line (the default)
```

Every action that takes a turn moves the clock on; TIME (which doesn't)
tells the player the time: "It's 8:52 am." Scripts and text read it
through `clock`:

| | |
|---|---|
| `clock.time` | `"8:52 am"` |
| `clock.hour`, `clock.minute` | 8 and 52 (hours from 0 to 23) |
| `clock.minutes` | Minutes after midnight: 532 |
| `clock.day` | The day, from 1 |
| `clock.night`, `clock.daytime` | Whether it's between dusk and dawn, or not |
| `clock.period` | `"morning"`, `"afternoon"`, `"evening"` or `"night"` |

```yaml
rooms:
  street:
    description: |
      ${if clock.night}The street lamps hum.${else}Shoppers hurry past.${end}

timers:
  sunset:
    at: dusk               # each day, when the clock gets there
    say: The sun slips behind the rooftops.

verbs:
  sleep: { patterns: [sleep] }
on:
  sleep: 'set_clock("6:00"); say("You sleep until dawn.")'
```

`pass_time(30)` moves the clock on 30 minutes, and `set_clock("6:00")` to
the next 6:00 (tomorrow's, if it's already later). Timers `at` a time go
off once each day the clock passes it, even when a script jumps over it.
The time is saved and undone with everything else.

The moon changes a phase a day: `clock: { moon: full }` is the first
night's (new, waxing crescent, first quarter, waxing gibbous, full,
waning gibbous, last quarter, waning crescent). TIME says it at night,
and scripts read `clock.moon`. A way out can be open only some of the
time: `west: { to: hollow, open_at: full moon, blocked: Only a blank
hillside. }`, or `open_at: night`, `day`, `22:00-4:00`, or a list that
must all hold.

In the web editor, choose **Add component → Time of day**, or **Keep the
time of day** under **Time of day** in the game's details. The Small House
starter template uses both goals and a clock.

### Routines

With a clock, characters can keep to a daily routine:

```yaml
characters:
  baker:
    name: baker
    in: bakery
    behavior: merchant
    routine: { "6:00": bakery, "13:00": square, "20:00": cottage }
```

Each turn, a character with a routine heads for the latest place whose
time has come, a room at a time, and stays there once it arrives. Before
the first time of the day, it's where the last one left it. A character
in a fight, running scared or under orders deals with that first, and a
hunter with prey in sight hunts. A routine needs a `clock:`.

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

In the web editor, add one with **Add component → Hint**, or under
**Hints** in the game's details. Each hint's panel has its steps, the
"Offered while" condition, and **Earlier**/**Later** to put them in
order. The editor's starter templates all have hints to copy from.

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
  see a thing.", and characters are just "Something": "Something
  attacks you." Moving still works, to feel your way out. LIGHT LAMP
  (TURN ON, SWITCH ON, IGNITE) and EXTINGUISH LAMP (PUT OUT, BLOW OUT,
  TURN OFF, SNUFF) are standard verbs for light sources; lighting one in
  the dark shows the room at once.

  **Feeling your way.** In the dark the player can use what they're
  holding (a lighter, a box of matches) and reach what they know is
  there, but nothing else ("It's too dark to see."). They know where
  something is once they've seen it there in the light, put it down
  there, or felt for it:

  ```
  > feel around
  You feel around in the dark: a brass key, a candle, a table and a crate.
  > take key
  Taken.
  > light the candle with the lighter
  ```

  FEEL AROUND (GROPE ABOUT) finds what's lying about and on things; FEEL
  FOR THE KEY (GROPE FOR, FUMBLE FOR) feels around and picks it up.
  Opening a box in the dark, or feeling in it, finds what's inside. LOOK
  in the dark reminds the player what they know is there, and EXAMINE
  says "It's too dark to see the brass key, but you can feel it there."
  Something with `findable_in_dark: true` can always be reached in the
  dark, seen or not: a light switch by the door, the matches by the bed.

  **Light from scripts.** A room's darkness can change: scripts read and
  set `world.cellar.dark`. When the light changes some way other than the
  player's own LIGHT or PUT OUT (a timer, a script, a lamp running dry),
  the room is described at once, or the darkness is. A flash of lightning
  that lights the cellar for one turn:

  ```yaml
  timers:
    storm:
      every: 3-8
      say: Lightning flashes through the grating!
      do: |
        world.cellar.dark = false
        start_timer("storm_dark")
    storm_dark:               # one turn later
      after: 1
      running: false
      do: world.cellar.dark = true
  ```

  Scenery that's only a light, a `light_source` with no `description`
  (moonlight through a window), lights the room but isn't something to
  feel for in the dark.

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

- **Making things:** `combines: { with: hook, makes: grapple }` on an
  object makes a third thing from two: TIE THE ROPE TO THE HOOK, COMBINE
  …, PUT … IN … and the like all do it (`by: [tie]` to say which verbs).
  Both are used up unless listed in `keep:`, and `say:` words it ("You tie
  the rope to the hook."). The made thing starts nowhere, with no `in:`.
- **Barricades:** PUSH THE WARDROBE AGAINST THE DOOR (or BARRICADE THE
  DOOR, with whatever `pushable` thing is there) shuts the door and blocks
  it both ways, for characters too, until it's moved: MOVE THE WARDROBE
  AWAY, UNBLOCK THE DOOR, or pushing it off to another room.
- **Leaving marks:** with something that `marks: true` (chalk, a knife),
  MARK, CHALK AN ARROW or SCRATCH "THIS WAY" leaves a mark of the player's
  in the room, one each. LOOK shows it ("Your mark is here: an arrow."),
  and so does MAP; RUB OUT THE MARK takes it away. For mazes.
- **Swaps:** a character with `watches: [ruby]` won't let the player take
  it while they can see ("The guard won't let you near the ruby."). A fake
  with `decoy_for: ruby` can be swapped for it unseen: REPLACE THE RUBY
  WITH THE GLASS GEM, or TRADE THE GLASS GEM FOR THE RUBY.
- **Distractions:** THROW THE PEBBLE WEST (or just THROW IT) and everyone
  here looks toward the noise ("The guard turns toward the sound."), for
  this turn and the next: long enough to take what they were watching, or
  slip past. Companions aren't fooled.
- **Mirrors:** with something that `reflects: true`, LOOK EAST WITH THE
  MIRROR (or LOOK AROUND THE CORNER) shows the next room, through an open
  way, without going in or being seen. ANGLE THE MIRROR NORTH sets it down
  to throw this room's light into a dark one next door, for as long as
  it's left there.
- **Reaching:** a thing that's `far: true` (a nest over a ravine) can't
  be reached from here at all ("too far off to reach"), and one that's
  `high` needs climbing. Holding something that `reaches: true` (a long
  pole) reaches both: HOOK THE NEST WITH THE POLE.
- **Taking apart:** `splits_into: [lens, oil]` (or `{ parts: [...], say:
  ... }`) on an object: TAKE THE LANTERN APART (DISMANTLE) leaves its
  parts where it was. The parts start with no `in:`.
- **Voices:** a character the player has talked to or overheard has a
  voice they can put on. CALL OUT AS THE CAPTAIN (IMPERSONATE, PRETEND TO
  BE, or IMITATE) brings anyone with `answers_to: [captain]` within two
  rooms, who can't see who's calling, hurrying to the voice: away from
  their post. If they find the player, they think less of them.
- **Planting evidence:** PLANT THE RUBY ON THE COOK (SLIP … INTO …) puts
  it in their pockets, if they don't notice (as for pickpocketing). When
  whoever it was stolen from, or a character that `watches` it, sees it on
  them, they take it back and turn on them ("Thief!"), and forgive the
  player.
- **Word gets round:** `friends: [baker]` on a character: when how it
  feels about the player changes, its friends (and those who count it a
  friend) hear of it three turns later, and shift a step the same way.
  Factions already share their own word, their attitude.
- **Remembering:** characters remember the last thing the player did to
  them: `gift`, `attacked`, `robbed`, `spared` (fled from a fight) or
  `fooled`. `remembers: { gift: "She smiles at the brooch.", robbed: "He
  keeps a hand on his purse." }` is said the next time they meet, once.
  Scripts read it as `world.guard.remembers`.
- **Playing dead:** PLAY DEAD (FEIGN DEATH, LIE STILL) and anything that
  doesn't eat the dead leaves the player alone ("The wolf sniffs at you,
  and loses interest."), until they do something besides waiting or
  looking about. A faction that `eats_corpses` isn't fooled.

[The advanced tutorial](docs/ADVANCED.md) builds a whole game around
factions, with all four tricks as ways through.

### How they feel about you

Every character has a **disposition**: how they feel about the player,
from -10 to 10. It's one of five feelings: hostile (-5 or less), wary,
neutral (the usual, around 0), friendly (2 to 4) and devoted (5 or more).
Set where it starts with a number or a word:

```yaml
characters:
  baker: { name: baker, in: bakery, behavior: merchant, disposition: friendly }
```

What already decides whether someone fights moves it too:

- Up: a gift they like (+2), an apology or a bribe (+1), taming
  (devoted), and their side warming to the player (+1 for each member).
- Down: an attack (-2), theft they notice (it counts as an attack), an
  insulting offer (-1), and their side souring (-1 each). Becoming
  someone's foe makes them hostile.

And it matters:

- EXAMINE says how they seem: "The baker seems glad to see you."
- Merchants charge a friend less (10% off, devoted 20%) and someone they're
  wary of more (20% more), and pay them more or less for things.
- At hostile, they won't talk ("The baker won't talk to you.") or trade,
  and one that fights back treats the player as a foe.
- In a conversation, `self` is the one being talked to, so an option can
  be for friends only. `why_not` shows it greyed, with the reason, while
  it's closed, and `disposition` on an option changes how they feel when
  it's chosen:

```yaml
choices:
  baker_talk:
    start: hi
    nodes:
      hi:
        text: '"Morning!" says the baker.'
        options:
          - { text: Compliment the bread, say: '"Why, thank you!"', disposition: 2, goto: hi }
          - text: Ask for the secret recipe
            when: self.disposition >= 4
            why_not: The baker doesn't trust you enough yet.
            say: '"Butter. Lots of butter."'
            end: true
          - { text: Goodbye, end: true }
```

```
> talk to baker
"Morning!" says the baker.
  1. Compliment the bread
  2. Goodbye
  – Ask for the secret recipe (The baker doesn't trust you enough yet.)

> 1
"Why, thank you!"
[The baker warms to you: friendly now.]
```

Scripts can read and set `self.disposition`, and read `self.feeling`
("friendly"). Setting it directly doesn't make anyone a foe; only the
game's own changes do.

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
same thing; `…` is where the name of a thing or person goes. AND joins
commands: TAKE THE KEY AND OPEN THE DOOR, as do THEN and full stops.

Players can also talk the way they would to a person, and the game
follows:

- **Lists and picks:** TAKE THE KEY, THE COIN AND THE LAMP; TAKE ALL BUT
  THE KEY; X MY HAT; X THE BRASS ONE; TAKE THE OTHER ONE; DROP THE SECOND
  KEY; TAKE TWO COINS. Two things that can't be told apart (two gold coins)
  don't get a "which do you mean?".
- **Extra words are fine:** CAN I GO EAST, TRY WEST, LET'S GO SOUTH, LOOK
  CLOSELY AT THE LAMP, GO EAST PLEASE, SAVE GAME, UNDO UNDO (twice), TAKE
  IT AGAIN.
- **Questions:** WHAT'S HERE, WHAT AM I CARRYING, WHO IS HERE, WHAT CAN I
  DO.
- **The room and the floor:** X ROOM, SEARCH HERE, TAKE THE KEY FROM THE
  FLOOR, PUT IT ON THE GROUND (drops it).
- **Small talk and the odd thing:** HELLO, JUMP, SIT, LIE DOWN, SLEEP,
  SING, DANCE, PRAY, CRY, LAUGH, THINK, NOD, SHRUG, HUG, CLAP, SHIVER,
  SIGH, YES, NO, THANKS, SORRY, XYZZY and the like get a short reply. Each is a verb you can hook (`on: { jump: ... }`)
  or reword with its `default`. HELLO KEEPER talks to the keeper.

#### Looking and moving

| Command | Also | What it does |
|---|---|---|
| LOOK | L, LOOK AROUND, WHERE AM I, X ROOM | Describes where you are. EXAMINE and SEARCH work on the room too: SEARCH HERE, LOOK AT THE HALL. |
| EXITS | WAYS OUT, WHERE CAN I GO | Lists the ways out of here. Takes no turn. |
| EXAMINE … | X …, LOOK AT … | Describes a thing or person. |
| SEARCH … | LOOK IN …, LOOK UNDER …, LOOK BEHIND … | Looks closely, and can turn up hidden things. |
| N, S, E, W, NE, NW, SE, SW, UP, DOWN, IN, OUT | NORTH, GO NORTH, CLIMB UP, … | Goes that way. |
| GO THROUGH … | ENTER … | Goes through a door. |
| SWIM NORTH | WADE …, PADDLE …, DIVE … | Swims, in a game with water. |
| GO TO … | WALK TO …, RETURN TO … | Walks to a place you've been, by the way you know, a turn a step. It stops if anything happens on the way, and UNDO takes the whole walk back. GO TO THE KEEPER goes to where you last saw them. |
| GO BACK | BACK, RETURN | Goes back to the room you just came from. |
| KITCHEN | GO KITCHEN, ENTER KITCHEN | A place's name on its own goes there: next door, or anywhere you've been. |
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
| TAKE … | GET …, PICK UP …, TAKE … FROM … | Picks something up. TAKE ALL, TAKE ALL BUT THE KEY, TAKE THE KEY AND THE LAMP. |
| DROP … | PUT DOWN … | Puts down something you carry. |
| PUSH … NORTH | SHOVE, DRAG, PULL, ROLL, SLIDE, MOVE … NORTH | Pushes something heavy into the next room, and follows it. |
| PUT … IN … / PUT … ON … | INSERT … INTO …, PLACE … ON … | Puts a thing into a container or onto a surface. |
| EMPTY … | EMPTY … INTO …, EMPTY … ONTO … | Tips out what's inside. |
| OPEN … / CLOSE … | SHUT … | Opens or closes a door or container. |
| UNLOCK … / LOCK … | … WITH … | Unlocks or locks it, with the right key. |
| WEAR … / TAKE OFF … | PUT ON …, REMOVE … | Puts clothing on or takes it off. |
| EAT … | | Eats something edible. |
| READ … | | What's written on it (its `text`), or a look at it. |
| COPY … | COPY … ONTO …, FORGE …, TRACE …, TRANSCRIBE … | Makes a copy, with paper and a pen; or copies the words into your notes. |
| FILL … FROM … | FILL … WITH …, DIP … IN … | Fills something that holds a liquid. |
| POUR OUT … | POUR … INTO …, POUR … ON …, EMPTY … | Pours a liquid out, into something, or on a fire. |
| TYPE 4512 | ENTER 4512 ON …, DIAL …, TURN … TO 4-7-1, OPEN … WITH 4512 | Enters a code. |
| LISTEN / SMELL | LISTEN TO …, SMELL … | What can be heard or smelt here, and from next door. |
| DRINK … | DRINK FROM …, SIP … | Drinks something `drinkable`. A drink you carry is used up; a fountain isn't. |
| CLIMB … | CLIMB UP …, CLIMB DOWN …, SCALE … | Goes up or down a way that goes `via` it, or climbs onto something you can stand on. |
| WAKE … | WAKE UP …, ROUSE … | Wakes someone who's asleep. |
| SHOW … TO … | SHOW … | Shows someone something (and frightens them, if they fear it). |
| SAY … | SAY … TO …, WHISPER … | Says something aloud, for your `on: { say: ... }` handlers (the words are `args.text`). |
| TIE … TO …, TIE UP … | ATTACH, FASTEN, LASH … TO …, BIND … | Ties a rope to something, or someone up with it. |
| UNTIE … | UNKNOT, UNBIND, FREE … | Unties a rope, or someone tied up. |
| KNOCK ON …, KNOCK ON THE WALLS | RAP ON, BANG ON …, SOUND THE WALLS | Knocks on a door (someone may open it), or listens for a hollow spot. |
| MEND … WITH … | REPAIR, FIX, PATCH, GLUE, SEW UP … | Mends what's worn or broken. |
| WRITE "…" ON … | SCRIBBLE "…" ON …, JOT DOWN … | Writes a note, with paper and a pen. |
| RIDE … | MOUNT …, CLIMB ONTO … | Rides a horse, if it'll let you. |
| DISMOUNT | GET OFF …, JUMP OFF … | Gets down off the horse. |
| ROW EAST | SAIL, DRIVE, STEER, PUNT, RIDE, GALLOP … | Goes, in a boat, a cart or on horseback. |
| CUT, DIG, WAVE AT …, POINT AT … | CHOP, SLICE … | Tries it out; the game decides if anything happens. |
| INVENTORY | I | Lists what you carry. |
| INVENTORY BY PLACE | WHERE DID I GET EVERYTHING | What you carry, by where you found it: "From the start: a pocket watch. From the Yard: a coin and a pebble." |
| USE … | USE … ON …, USE … WITH … | Does what the thing is for: lights a lamp, wears a hat, eats food, opens a door (or goes through it once it's open). USE KEY ON DOOR unlocks it, and USE SWORD ON GOBLIN attacks. |
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
| ASK … THE WAY TO … | WHICH WAY TO …, HOW DO I GET TO …, ASK FOR DIRECTIONS TO … | Asks the way to a place: after that, GO TO goes there. |
| PROMISE … | I'LL DO IT, PROMISE | Promises someone the favour they asked. |
| TALK TO … | SPEAK TO …, GREET …, ASK …, TELL …, ASK … ABOUT …, TELL … ABOUT … | Starts a conversation. With one person here to talk to, TALK or ASK ABOUT … needs no name. ASK THE KEEPER ABOUT THE GATE goes straight to the option that mentions the gate, when just one does. |
| GIVE … TO … | HAND … TO …, FEED … TO … | Hands something over: a gift, or food for an animal. |
| LET … SNIFF … | HAVE … SMELL …, GIVE … THE SCENT OF … | Gives a dog a scent to follow. |
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
| WAIT 3 | WAIT 10 MINUTES, WAIT AN HOUR, WAIT UNTIL DUSK, WAIT UNTIL 9:30 PM | Waits a while, stopping as soon as anything happens. Minutes, hours and times of day need a [clock](#time-of-day). |
| AGAIN | G | Repeats your last command. AGAIN WITH THE IRON KEY, or AGAIN, THE PEAR, repeats it with something else. |
| UNDO | UNDO 3, TAKE BACK 3 | Takes back your last move, or several. |
| OOPS … | | Fixes the word the game didn't know in your last command: after TAKE THE LANTREN, OOPS LANTERN. When a slip is too close to more than one word to fix by itself, the game asks "Did you mean TAKE?": answer YES, or the word. |
| SAVE / RESTORE | LOAD | Saves your place, or goes back to a save. |
| RESTART | | Starts the game again. |
| SCORE | | Shows your score, if the game keeps one. |
| HELP | | A short guide to playing. |
| MAP | WHERE HAVE I BEEN, PLACES | The places you've been, in words, here first, and the ways out of each: where they lead, or that you haven't been that way yet, and the doors on them. For the terminal and screen readers. With a map in the game in view, MAP looks at that instead. |
| WHAT DID SHE SAY? | WHAT DID THE KEEPER SAY?, WHAT WAS SAID, CONVERSATION | Shows the last conversation again (or the one with someone named), what you chose too, without opening it. Works in the middle of one. |
| RECAP | SUMMARY, REMIND ME, WHERE WAS I | Where things stand: where you are, what you carry, your score and goals, and your last few moves. For coming back to a game after a while. |
| WHERE IS … | FIND …, WHERE DID I LEAVE …, WHERE DID I GET … | Where you last saw something or someone: "You last saw the old keeper in the Base of the Lighthouse." For something you carry, where you found it. GO TO THE KEEPER walks you there. A reminder, not a hint: it only knows what you've seen, so it never tells you where something has gone since, or what's become of it. |
| BRIEF / VERBOSE | | BRIEF names places you've been before without describing them again (LOOK still does); VERBOSE, the default, always describes them. Kept with saves, and through UNDO and RESTART. |
| HINT | HINTS, CLUE | The game's next hint for where you're stuck, if it has hints. |
| JOURNAL | GOALS, OBJECTIVES, TASKS, QUESTS | Your goals, if the game has them, your promises, and your notes. |
| WHAT DO I KNOW | WHAT HAVE I LEARNED, CLUES, RECALL, WHAT DO I KNOW ABOUT … | What you've been told or read that matters, and who told you. Takes no turn. |
| NOTE … | JOT DOWN …, WRITE DOWN …, MAKE A NOTE … | Writes a note of your own, word for word: NOTE The code might be 4512. Takes no turn. |
| NOTES | MY NOTES, NOTEBOOK | Your notes, numbered. ERASE NOTE 2 rubs one out; CLEAR NOTES rubs out all of them. |
| TIME | WHAT TIME IS IT | The time of day, if the game keeps one. |
| WHO IS HERE | WHO'S HERE, IS ANYONE HERE | Who else is in the room. |
| WHAT CAN I DO | WHAT NOW, OPTIONS | A few things worth trying here, right now. |
| ABOUT | INFO, CREDITS, VERSION | The game's title, author and version. |
| COMBINE … WITH … | JOIN, CONNECT, MIX, FIT … TO … | Puts two things together, if they make something (TIE and PUT IN can too). |
| PUSH … AGAINST … | BARRICADE …, BLOCK … WITH … | Blocks a door with something heavy; MOVE … AWAY or UNBLOCK … clears it. |
| MARK | CHALK …, SCRATCH …, LEAVE A MARK | Leaves a mark here, with something to mark with: CHALK AN ARROW. RUB OUT THE MARK takes it away. |
| REPLACE … WITH … | SUBSTITUTE … FOR …, SWITCH … WITH … | Takes one thing and leaves another in its place. |
| PLAY DEAD | FEIGN DEATH, LIE STILL, PLAY POSSUM | Lies still until you do something else. |
| QUIT | Q | Stops playing. |

In the browser, Up and Down go back through the commands you've typed,
and **Tab finishes a word** from what you can see: TAKE LA and Tab gives
TAKE LANTERN. With several matches it puts in the first and shows them all
under the line; Tab again (Shift+Tab back) goes through them, and Esc puts
back what you typed. It only offers what you can see or carry, places
you've been, and the game's commands, never something you haven't found.

After an ending, **Look back over the story** steps through it a turn at
a time (◀ and ▶, or the arrow keys), marking each turn in the story and
fading what came after. **Play on from here** takes the story back to
that turn and carries on, as if the rest hadn't happened.

Text and colours has ways to make reading easier: the OpenDyslexic
typeface, the "Roomiest" line spacing (more space between letters and
words too), and text that appears **a sentence at a time**, Enter for the
next. Screen readers still get the whole of it at once.

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
`messages`, `aliases`, `scripts`. Rooms, objects and characters can have a
`picture` (`{ file, alt }`).

**Traits:** `portable`, `scenery`, `container`, `supporter`, `openable`,
`lockable`, `door`, `wearable`, `edible`, `drinkable`, `light_source`, `enterable`,
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
can have `damage` (weapons), `kills`, `price`, `disguise`, `eaten_by`, `nameable`, `calls`, `trap`, `douses`, `rope`, `anchor`, `hollow`, `mends`, `breaks_into`, `spoils`, `two_handed`, `warm`, `tries`, `feedback` and (worn) `sees_in_dark`; characters a `password`, `reads`, `mount` and `stamina`; objects a `vehicle`, `flashback` and `now`; rooms can be `cold` or a `crowd` (and `now`), and exits need a `rope` or go `by` something; top-level `flashbacks:` and `epilogue:`; the player can have `health`,
`damage`, `money`, `voice`, `sees_in_dark`, `hands` and `warmth`. Events: `defeated` on a character, `died` at the
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
`stop_timer(id)`, `timer_left(id)`, `goals.<id>`, `start_goal(id)`,
`complete_goal(id)`, `clock.time`, `clock.hour`, `clock.night`,
`clock.day`, `pass_time(minutes)`, `set_clock(time)`; on objects,
`:move_to(place)`, `:is_in(holder)`, `:contents()`, `:has(trait)`,
`:can_hold(thing)`, `.capacity`, which can be changed, and `.seen`,
whether the player has found it yet (seen it, or held it), for hints.

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
  game from, in their browser, a single web page that plays anywhere,
  even offline, and getting it into the gallery.
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
