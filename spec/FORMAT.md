# EIFGB Source Format: Draft 0.1

Status: **draft for discussion**. The worked example is `examples/lighthouse/game.yaml`.

## Pipeline

```
source (YAML + Lua) → validate → compile → Story IR (JSON) → target runtime
                                                               ├─ web: HTML + WASM engine
                                                               └─ desktop: native binary (later)
```

The source format is what people write, and it can change for their
convenience. The IR is what the engine reads: it is normalized, has no
syntax sugar, and carries its own version number. The web editor reads
and writes the source format, so files edited in the editor and by hand
stay interchangeable.

## Files

- A project is a folder with a `game.yaml` entry file.
- `scripts:` lists `.lua` files to load.
- *(Planned)* `include:` will let large games split rooms, objects and so on across several YAML files.

## Top-level sections

| Section      | Purpose |
|--------------|---------|
| `format`     | Format version, e.g. `eifgb/0.1`. Required. |
| `game`       | Metadata, start room, intro text, max score. |
| `presentation` | The website's look: `theme` (one of `light`, `dark`, `sepia`, `terminal`, `amber`, `contrast`) and `accent` (`#rgb` or `#rrggbb`). Mistakes are warnings, and dropped. Carried in the IR as `meta.presentation`; the engine ignores it, and the web player applies it while the player's own colours are on "auto". |
| `settings`   | Game-wide switches: `describe_state` (default `true`), `track_life` (default 30) and `list_exits` (default `true`). |
| `vars`       | Global story state (scalars, lists, maps). |
| `player`     | Player description, carrying capacity, `health`, `damage` and `money`. |
| `rooms`      | Locations and their exits. |
| `objects`    | Things. Each starts somewhere via `in:`. |
| `characters` | NPCs and enemies: things that talk, move, fight and trade. See **Characters**. |
| `verbs`      | Custom verbs: grammar `patterns`, a `default` reply, and `reach` (default `true`). An author verb's leading words take priority over stdlib synonym phrases. |
| `choices`    | Choice graphs, used for menus and dialogue. |
| `on`         | Global event handlers (`turn`, `start`, ...). |
| `endings`    | Named endings with a `kind` (`win`, `loss` or `neutral`). |
| `timers`     | Things that happen after a number of actions. See **Timers**. |
| `factions`   | Sides for characters: `hostile_to` (factions and/or `player`) and `eats_corpses`. See **Characters**. |

IDs are `snake_case`. Rooms, objects and characters share one namespace. Verb IDs can't be event names (`start`, `turn`, `parse_failed`, `died`, `enter`, `leave`, `defeated`), because handlers are keyed by both.

## Entities

- **Exits** take one of three forms. A room ID is a plain exit. A string is a blocked exit, with that string as the message. A map `{to, through, when, blocked}` handles doors and conditional exits.
  A direction can also take a **list** of those forms, for several exits the same way. When the player goes that way, entries with `to` whose `when` holds (or that have no `when`) are candidates:
  - **One candidate:** the player goes there.
  - **Several, each through a different door:** the game asks "Which do you mean, the red door or the blue door?". The reply runs as GO THROUGH that door, and asking doesn't take a turn.
  - **Several without distinct doors:** the first one wins, and the compiler warns that the rest can't be reached.
  - **None:** the first `blocked` message in the list is shown, or a default.

  So a conditional route is `[{to: bridge, when: vars.bridge_down}, {blocked: "The bridge is raised."}]`. GO THROUGH, WALK THROUGH, ENTER and GO INTO a door also leave by the exit that uses it.
- **Containers and supporters.** These are the `container` and `supporter` traits, with `in:` placing things inside or on top:
  - **Visibility:** a closed container hides its contents, unless it's `transparent`, in which case they're visible but out of reach: "You can't reach the jewel: the glass case is closed." `[reach.closed]`
  - **Reach:** every verb carries a `reach` flag in the IR. When it's set, a target or tool out of reach stops the command before any handler runs. The built-in verbs that touch things need reach (TAKE, OPEN, PUT, EAT…); EXAMINE, SEARCH, LOOK and TALK don't. Author verbs and the stdlib's try-it verbs need it by default. LISTEN doesn't (`reach: false` in the stdlib), and authors opt out per verb with `reach: false`. A container without `openable` is always open. Characters' belongings are never in scope, except a merchant's priced goods for BUY and EXAMINE.
  - **Descriptions:** LOOK, EXAMINE and SEARCH describe contents as deep as the player can see. The inventory lists carried things, then one sentence per carried thing the player can see into, recursively: "In the cloth bag is a coin." `[inventory.in, inventory.on]` (with `args.count` for is/are).
  - **TAKE X FROM Y** checks that X is in or on Y, at any depth: "The coin isn't in the glass jar." `[take.not_in, take.not_on]`.
  - **ALL with a second object:** TAKE ALL FROM Y means what's in Y. PUT ALL IN/ON Y means what you carry, except Y itself, anything containing Y, and worn things.
  - **EMPTY X** tips X's portable contents where X is, or where the player is if they're holding X. EMPTY X INTO/ONTO Y moves them to Y, as far as they fit.
  - **`enterable`** (needs `container` or `supporter`): GET IN/INTO/ON, CLIMB IN/INTO/ON, SIT/STAND/LIE ON, HIDE IN to enter. GET OUT/OFF/UP, CLIMB OUT/OFF, STAND UP and OUT to leave.
    - The room title becomes "Hall (in the wardrobe)".
    - A closed opaque container the player is in becomes their view: "You're shut inside the wardrobe." They can't see or reach out.
    - Moving away, or climbing into something else, gets out first, opening the container if needed.
  - **`pushable`:** verb `push_to` (push/shove/pull/drag/move/roll/slide/heave {target} {direction}). The thing must be directly in the room `[push.stuck]`, not carried `[push.carried]`, and the player not in or on it `[push.on_it]`; not pushable → `[push.cant]`. Then it goes exactly as GO in that direction (doors opened or refused, `when`, `blocked`); when the player arrives, the thing moves too and `[push.done]` comes before the room description. TAKE on something pushable and not portable → `[take.pushable]`. CLIMB {direction} is GO.
  - **`high: true`** (objects): it and everything in or on it are out of reach, `[reach.high]`, for every verb that needs reach, unless the player carries it, is in or on it, or is on a supporter (STAND ON, CLIMB ON, CLIMB {target} enter it). Checked with `reach.closed`, before any handler.
  - **Size and weight:** `size` (default 1) and `capacity`. A thing's weight is its size plus the weight of everything in or on it. With a capacity, the total weight of what's directly in or on the holder can't go over it; a thing whose size fits but whose weight doesn't gets "too heavy … with everything in it" `[take.too_heavy, put.too_heavy]`. The player's capacity counts what they carry: `player.capacity` is a whole number, where `0` means they can't carry anything (`take.cant_carry`), or `none`/null/omitted for no limit (the default). Without a capacity, a thing can't go in or on something smaller than itself, but the player can carry anything. Replies are "too big for" `[put.too_big, take.too_big]` and "no more room" `[put.full, take.full]`. The compiler warns when a holder starts out over capacity.
- **Hidden things** (`state: { hidden: true }`) can't be seen or referred to. Things the player carries are the exception. `revealed_by` reveals one without a script:
  - **The rule** is `"<verb> <object>"` (`search mat`, `open desk`, `kick rug`), or a list of rules where any one will do. Any phrasing of the verb counts.
  - **Action categories:** a rule can name a stdlib category instead of a verb: `"(physical action) rug"`. Case and spaces/underscores in the name don't matter. Categories are expanded into verb keys at compile time and stored in the IR with the category name. That pins them to the game's stdlib version, so a later stdlib that adds a verb to `(physical action)` doesn't change an existing game. v1 has:
    - `(physical action)`: push, pull, kick, hit, move, lift, shake, twist, touch
    - `(inspect)`: examine, search
    - `(sense)`: smell, listen, taste, touch
  - **What the player sees:** when the action happens (and isn't blocked by a `before_` handler), the thing becomes visible where it is, and the game says "You find a small key." `[reveal.found]`. For SEARCH, and for custom verbs with no handler, that replaces the usual reply. Otherwise it follows it.
  - **Secret doors:** a hidden door's exits aren't listed or usable until it's revealed. Like `blocked`, a door's `hidden` can be `true` (both sides), a room ID or a list of room IDs (hidden only from those sides). That covers a bookcase door hidden from the library but plain from the secret room. Revealing a door clears `hidden` entirely, and so does passing through it from a side where it's visible.
  - **Compiler checks:** the verb and object must exist (with "did you mean"), the rule must have exactly two words, and it can't name the hidden thing itself. There's a warning if the thing doesn't start hidden.
  - **Scripts** can still reveal by setting `hidden = false`, for anything more involved.
- **Doors work from both sides.** A door is an object with the `door` trait. It's placed in one room with `in:`, and exits go `through:` it. For every exit from room A to room B through a door, B's way back to A uses the same door:
  - **An exit back to A through the door exists:** nothing changes.
  - **Exactly one plain exit back to A exists:** the door is attached to it.
  - **There's no way back:** one is added in the opposite direction (north ↔ south, up ↔ down, in ↔ out, and so on).
  - **That direction already leads elsewhere:** the compiler warns and leaves the map alone.

  The door is in scope, listed and described in every room it connects, so it can be examined, opened, closed, locked and unlocked from either side. Its `in:` only decides which room `:is_in` and scripts see as its location.
- **Blocked doors are held shut.** The `blocked` state means barred, bolted or wedged, so a blocked door is always closed:
  - `true`: it won't open from either side. "The trapdoor won't open."
  - A room ID, or a list of room IDs: it won't open from those sides. "The trapdoor won't open from this side." This applies to both OPEN and walking through.
  - From a side that isn't blocked, it opens normally. Opening it, or walking through it, lifts the block (the bar is on that side), and the way is then clear in both directions.
  - `false`, or no value: not blocked.
  - **Rules enforced:** blocking an open door closes it. The engine enforces that after every script, and the compiler refuses a door that starts open and blocked.

  The compiler also checks that the named rooms are on a side of the door. Scripts change `blocked` like any state (`world.trapdoor.blocked = "cellar"`), and it's saved and undone with everything else. Automatic descriptions show it from the player's current side: "It's closed. It's blocked from this side." and "a trapdoor (closed, blocked from this side)". On things that aren't doors, `blocked` is an ordinary author state.
- **Automatic state descriptions** (`settings.describe_state`, on by default). The engine describes state itself, for both the built-in states and any state fields an author adds:

  | State | When examining | In lists | In room descriptions (scenery only) |
  |---|---|---|---|
  | open / closed / locked | "It's locked." | "an iron gate (locked)" | "The pantry door is locked." |
  | `worn` | "You're wearing it." | "a scarf (worn)" | *(none)* |
  | empty (a container the player can see into, with nothing visible in it) | "It's empty." | "a small box (empty)" | "The cupboard is empty." |
  | heavy (portable, size within the player's capacity, weight over it) | "It's too heavy to lift with everything in it." | "a crate (heavy)" | same as examining, with the name |
  | `fueled: true` | "It's fueled." | "a lamp (fueled)" | "The great lamp is fueled." |
  | `fueled: false` | "It isn't fueled." | *(none)* | *(none)* |
  | `channel: 16`, `color: red` | "Its channel is 16." | *(none)* | *(none)* |

  - **Known things only:** list tags, room-description sentences, and the derived `empty` and `heavy` notes, appear only for things in `GameState.known`. A thing becomes known when the player EXAMINEs, SEARCHes or LOOKs IN it; OPENs, CLOSEs, LOCKs, UNLOCKs, WEARs, TAKEs OFF or EMPTYs it; tries to go through it (doors); or names it as the second object of PUT IN/ON, TAKE FROM or EMPTY INTO. Things that start in the player's inventory are known. Examining always describes every state, since examining makes the thing known.
  - **One sentence per thing in rooms:** when a scenery thing has several states to mention, they're joined from their tags: "The cupboard is open and empty." `[state.here.several]`
  - **Field names become words:** `_` turns into a space, and a leading `is_` on a true/false field is dropped, so `is_lit` reads "It's lit." Adjectives make the best names: `lit`, `broken`, `fueled`.
  - **Private fields:** a field whose name starts with `_` (`_visits`) is never described. Use that for bookkeeping.
  - **Not described:** lists and tables, and `hidden`.
  - **Fields scripts add during play** are described as well.
  - **No repetition:** if a description already reads a field in a template (`self.fueled`, `world.door.locked`), that field isn't described again. For open and locked, reading either one counts.
  - **Wording** comes from the `state.*` messages, so it can be reworded.
- **Traits** give built-in behaviour: `portable`, `scenery`, `container`, `supporter`, `open`, `openable`, `lockable`, `door`, `wearable`, `light_source`, `edible`.
- **State** is a map of mutable fields on an entity. Built-in traits read these fields, e.g. `open`, `locked` and `hidden`.
- **Parser vocabulary** comes from `name`, `synonyms` and `adjectives`. The adjectives are used for disambiguation.

## Text templates

All prose fields allow templates:

- `${expr}` inserts the value of a Lua expression.
- `${if cond}...${else}...${end}` includes text conditionally.
- Inside a template, `self` is the entity that owns the field.

## Scripting (Lua)

Handlers live under `on:`, keyed by event or verb name:

- `before_<verb>` runs before the default behaviour. Returning `block(msg)` stops it.
- `<verb>` replaces the default behaviour of a custom verb, or of a built-in verb on that object.
- `after_<verb>` runs after the default behaviour succeeds.

Handlers on the target object (the direct object) run first, then the room's, then the global ones.

A handler can be a block of statements or a single expression.

**API (implemented in `eifgb-lua`):**

- **Functions:** `say(...)` (also `print`), `block(msg?)`, `score(n)`, `end_game(key)`, `open_choice(key)`, `random(a, b)` (or `random(n)` for 1..n).
- **Text helpers:** `the(x)`, `The(x)`, `a(x)`, `A(x)`. These come from a module per language.
- **Names:** `world.<id>`, `player`, `here` (the current room), `vars.<name>`, `game.turn`, `game.score`, `game.max_score`, `game.title`.
- **Arguments:** `args.target`, `args.tool`, `args.text`, `args.direction`.
- **Fields on every entity:** `key`, `name`, `kind`, `location`. These are read-only; use `:move_to` to change `location`.
- **`capacity`:** readable and writable on every entity, starting from the source `capacity`. Reads give an integer or `nil`. Writes take a whole number ≥ 0, or `nil`/`"none"` for no capacity (unlimited for the player, the size rule for containers and supporters); anything else is a script error. Lowering it doesn't move anything out. It's saved and undone with the game, and because it's built in it can't be used as a state field name.
- **Character methods:** `:wander()`, `:step_toward(target)`, `:go(direction)`, `:attack(target, weapon?)`, `:eat(corpse)`, `:behave()` (each returns whether it did something, and its event is described like a behaviour's), `:is_with(other)`, `:is_hostile_to(other)`, `:distance_to(target)` (moves needed, or `nil`). See **Characters**.
- **Methods on every entity:** `:move_to(place)` (ignores size and capacity), `:is_in(holder)`, `:contents()`, `:has(trait)`, `:can_hold(thing)` (whether `thing` would fit in, on or be carried by this, by size and capacity; true if it's already there).
- **State fields** are read and written directly (`self.fueled = true`). Setting one to `nil` removes it. The built-in field and method names above are reserved and can't be used as state fields.
- **Each entity has exactly one Lua object**, so `==` works as expected (`args.tool == world.oil_can`).
- **`vars` must be declared** in the `vars:` section. Reading or writing an undeclared name is an error, which catches typos like `vars.lamp_lt`.
- **Stored values** can be booleans, numbers, strings, lists and string-keyed tables. Objects can't be stored; store `x.key` instead. Tables are stored as copies, so to change one, assign the whole table back: `local l = vars.log; table.insert(l, x); vars.log = l`.

**Effects.** Scripts change world state immediately. Everything else they do (printing, scoring, endings, opening a choice) is queued as an effect, and the engine handles it, in order, once the script returns.

### Sandbox rules (these matter for an online service and for saves)

- There is no `io`, `os`, `require`, `load`, `dofile` or `debug`. `string`, `table` and `math` are available, but `math.random` is replaced by the engine's seeded `random()`.
- Each handler runs on an instruction budget (5M instructions), so runaway loops get aborted with an error message. `pcall` can't swallow that error. The whole VM also has a 64 MB memory limit.
- Reading an undefined global is an error, not `nil`, so a misspelled function name fails loudly.
- **Story state lives only in `vars` and in entity state.** Script globals are for functions. That rule is what makes save, restore and undo possible: saving is just serializing `vars` and entity state.
- On the server, scripts are only parsed and checked during compilation, never executed. They run on the player's machine. *(Optional later: a sandboxed automated playtest.)*
- **Checking:** `eifgb check` (and the web editor's checker) loads the script files, then compiles every Lua snippet (handlers, `when`, `do`, and `${…}` in text) the way it will be at play time, without running it. A syntax error is reported at the snippet's source path: `error: rooms.cellar.on.enter: Lua syntax error, line 2: ')' expected near <eof>`.

## Timers

A timer fires after a number of **actions**: commands that took a turn.
Parse failures and meta commands (SAVE, UNDO, SCORE…) don't count.

- **Schedule:** `after: N` (once) or `every: N` (repeating), exactly one. N ≥ 1.
- **Start:** `running` (default `true`, or `false` when `started_by` is given) starts it with the game. `started_by` and `stopped_by` take the same rules as `revealed_by`, including categories. They're checked when the action happens and isn't blocked by a `before_` handler. Starting a running timer leaves it alone.
- **Counting:** a timer started during an action counts the actions after it. One started outside an action (the `start` handler, or a choice) counts from the next action.
- **Firing:** due timers fire at the end of the turn, after `turn` handlers, in declaration order. `when` (Lua) is checked first; if false, the firing is skipped, and an `after` timer is then done. Then `say`, `do`, and `end` (an ending ID). Firing stops once an ending is requested.
- **Noise:** `noise: { at, loudness }` makes a noise when the timer fires (after `do`, before `end`). `at` is an entity ID: a room, or a thing whose room is used at the time; omitted, the player's room. See **Characters** for what hears it. `loudness` ≥ 1. A timer with only a `noise` isn't "doing nothing".
- **Scripts:** `start_timer(id)`, `stop_timer(id)` (both take effect when the script returns), and `timer_left(id)`, the number of actions left counting the current one, or `nil`.
- **State:** running timers live in `GameState.timers` (ID → due turn), so they're saved and undone.
- **Compiler checks:** schedule shape, ending IDs, rules, and literal `start_timer`/`stop_timer`/`timer_left` names. A warning is given for timers that do nothing.

## Characters

Every character has a behaviour, and takes a turn after each action that
counts for timers, in declaration order, before `turn` handlers and timers.

- **`behavior`:** `stationary` (default without `talk`), `conversationalist` (default with `talk`; never moves), `wanderer`, `merchant`, `hunter`, `guard`, `follower`, `defender` (never starts a fight; fights back at once). Unknown names are errors with a suggestion; a conversationalist without `talk` is a warning.
- **`wander`** (0–100): percent chance of moving to a random neighbouring room on a turn with nothing better to do. Defaults: wanderer 50, merchant and hunter 25, others 0. Merchants never wander while the player is with them.
- **`area`:** room IDs the character keeps to. **`range`:** rooms away at which a hunter or follower notices the player (default unlimited), by the shortest route it can take.
- **Pattern slots** must be separated by a word, except that `{direction}` (always one known word) can sit next to another slot, and `{text}` can come straight after another slot as the pattern's last token ("call {target} {text}"); the parser then tries the longest object first.
- **Listing exits:** a room description (outside the dark, and not shut inside something) ends with `[look.exits]` ("Known exits: north and up.") or, with none, `[look.no_exits]`, unless `settings.list_exits` is false; a room's own `list_exits` overrides it either way. Listed: each direction once, in source order, with an exit that has a `to`, isn't through a hidden door, and whose `when` (if any) is true now. Blocked-message-only exits are never listed.
- **Darkness:** a room is lit unless it's `dark`, in which case it's lit only while something in it (`room_of`) is a burning `light_source` (`lit` or `on` true) in plain sight. The player is in the dark when their surroundings aren't lit and they can't see in the dark. Their surroundings are the room, or, when shut inside something, also that enclosure, lit by a burning light inside it that's visible from within. The player sees in the dark with the `sees_in_dark` state (`player: { sees_in_dark: true }`, or set by scripts) or while wearing an object with `sees_in_dark: true` (the IR's `night_vision`; a warning if it isn't `wearable`). In the dark: LOOK gives `[look.dark_title]` and `[look.dark]` only; scope is the player and what they carry; a noun that isn't found gives `[parser.dark]`; characters in narration are "something". Verbs `light` (light/ignite/turn on/switch on {target}, light {target} with {tool}) and `extinguish` (extinguish/put out/blow out/snuff/douse/turn off/switch off {target}) need reach and set `on` if the thing has that field, else `lit`: `[light.done, light.already, light.cant, extinguish.done, extinguish.not_lit, extinguish.cant]`. When that changes whether the room is lit, the room is described (or the darkness is). Object handlers on `light`/`extinguish` replace the built-in, as for any verb.
- **Seeing in the dark:** a character sees another (or the player) only if the player isn't hidden (for the player) and the target's room is lit, or the character has `sees_in_dark` (on itself or its faction). This governs every "sees": hunters' and guards' targets and `_last_seen`, tamed characters' enemies, fighting back against foes, factions attacking on sight, witnesses, and finding bodies (a body in an unlit room is found only by one that sees in the dark).
- **Peeking:** verb `peek`: peek/peer/look {direction}, peek/peer/look through {target}, peek into {target}, peek/look through the keyhole of {target}; `reach: false`; takes a turn; wakes nobody. Through a door: the exit from here through it (open or not); not a door → `[peek.not_door]`. In a direction: the first visible exit that way with a `to`; none → `[peek.no_way]` (free). One paragraph: `[peek.through]` / `[peek.direction]` (with `args.place`, the room's name, only if visited); then, if the room isn't lit and the player can't see in the dark (see **Darkness**), `[peek.dark]` alone; else `[peek.empty]` (no characters, bodies or loose non-scenery objects), active non-hidden characters `[peek.one, peek.two, peek.several]` with `[peek.asleep_one, peek.asleep_all, peek.asleep_one_of_two, peek.asleep_some]`, `[peek.bodies]`, loose objects `[peek.few]` (1–2), `[peek.some]` (3–5), `[peek.lots]` (6+), and `[peek.lit]`.
- **Fear and morale:** `morale` (percent, 0–100; character, else faction, else 0) and `fears` (object IDs or `fire`; character and faction lists combined; the compiler checks them). At the very start of a character's turn: a panic counter (`_fleeing`) counts down; then its threat is the player, if it's frightened (`_fleeing` > 0) and with the player; or, with morale above 0, the first enemy in its room that it can see (hostile either way) when it's demoralized (health at or below `morale`% of full) or outnumbered (more such enemies than itself plus same-faction members there). With a threat it flees through an exit (by `npc_exits`) to a room the threat isn't in, preferring one without its enemies: "X flees north!" `[npc.flees]`; with no way out it's cornered and acts as usual (fights). With no threat, a frightened or demoralized character does nothing. BRANDISH X (AT Y) (verb `brandish`: brandish/wave {target} (at {tool}), thrust/swing {target} at {tool}, show {target} to {tool}): X must be held; every awake, active character in the room (or just Y) that can see the player and fears X panics for three turns `[npc.afraid]`, else `[brandish.nothing]`. Frightened characters get the always-shown tag "frightened" `[state.frightened]`. A demoralized or frightened companion refuses attack orders `[order.too_scared]`.
- **Orders:** verb `order` (tell/order/ask/command/instruct {target} to {text}; `reach: false`), and input of the form "NAME, rest" whose part before the first comma names exactly one character in scope, which is rewritten as "tell NAME to rest". The character must be active and awake `[order.not_person, talk.asleep]` and obey: tamed or `obeys: true` `[order.ignored]`. The text's first word is the order, the rest a noun phrase (matched in scope, then, for attacks, among all active characters) or a direction: attack/kill/fight/bite/maul/sic/get/charge a character (not the player `[order.wont_attack_you]`, not if pacifist `[order.pacifist]`) `[order.attack]`; fetch/bring/retrieve/get a thing `[order.fetch]` (as for a thrown thing); stay/wait/sit/halt/stop/guard/down `[order.stay]`; follow/come/heel/return `[order.follow]`; go/run/head/move/walk {direction}, or a bare direction `[order.go]`; drop/release/leave `[order.drop, order.nothing_to_drop]`; anything else `[order.confused]` (free). The standing order is the private `_order` state. On its turn, an obeying character with an order acts on it before anything else: attack (strike if together and it can see the target, else step toward it; done when the target's gone or unreachable), go (one move, then stay), stay (attack an enemy of the player here, else nothing).
- **Chatter and eavesdropping:** `chatter:` on a character is a list of lines: a string, or `{ say, loud, when, repeat }` (`say` is a template with `self` the speaker). After characters' turns, for each room at most one line is said, by the first character (declaration order) that's active, awake, has another active, awake character in the room, and can't see the player there. Its line is the first from where it left off (`_chatter`) that the player hasn't heard (`_said`), unless `repeat`, and whose `when` holds. Heard in the player's room as `[chatter.overheard]` (speaker and listener named, "someone" in the dark); from an adjoining room as `[chatter.nearby]` (`args.from` from `[from.<direction>]`), if `loud` or the player is eavesdropping on that room. Verb `eavesdrop` (eavesdrop (direction); listen {direction}; listen at/through {target}; press/put ear to {target}; listen in; `reach: false`) listens to the rooms behind a door, that way, or every adjoining room, this turn: `[eavesdrop.door, eavesdrop.direction, eavesdrop.all]`, then `[eavesdrop.nothing]` if nothing was heard; no such room `[eavesdrop.no_way]` (free).
- **Pickpocketing:** verb `steal` (steal/pickpocket/pinch/lift {target} from {tool}; steal {target}; pickpocket/rob {target}, where the target is the victim); TAKE X FROM Y with Y an active character is the same. For TAKE/STEAL with a character as the second object, that character's non-hidden belongings are in scope; for STEAL with none, everyone's here. PICKPOCKET picks the victim's most valuable (by `price`) non-hidden thing it could take. Refusals: no victim `[steal.nobody]`, nothing to take `[steal.nothing]`, not its `[steal.not_carrying]`, worn by an awake victim `[steal.worn]`, too heavy (take messages). The victim notices unless it's asleep, ate this turn or last (`_busy`), can't see the player, or is fooled: a member of the faction the player is disguised as, with attitude above 0, that doesn't have the player as a foe. Noticed: `[steal.caught]`, a merchant victim refuses to trade, and the victim is provoked as by an attack without damage (patience, foes, faction attitude if witnessed). Otherwise the thing moves to the player: `[steal.done]`, or `[steal.done_asleep]`.
- **Sleep:** the `asleep` state on a character (true/false, never described generically; tag "asleep" always shown, `[state.asleep]`). A sleeping character skips its turn entirely (no behaviour, `act` or body discovery), isn't a merchant here, and answers TALK with `[talk.asleep]`. It wakes (asleep removed; "X wakes with a start!" `[npc.wakes]` if in the player's room) when: the player arrives by walking (not SNEAK), after the room is described; a noise's loudness reaches it (by its route); or anyone is attacked in its room. An attack on a sleeper does double damage, with "You catch X asleep!" `[attack.sneak]` first when the player strikes. SNEAK/CREEP/TIPTOE {direction} (verb `sneak`) moves exactly like GO but doesn't wake anyone.
- **Bodies:** a dead character can be taken (by weight, ignoring `portable`), and PUT IN/ON (also hide/stuff … in) works on a body lying in the player's room. At the start of each character's turn, before its behaviour or `act`, if it has a faction and a body of that faction killed by the player (`killed_by: player`) lies in its room in plain sight (no closed, see-through-less container around it; carried by the player counts as in sight), and not yet `_found`: the body is marked `_found`, the faction's attitude sours by one (toward the player's current side), there's a noise of loudness 2 there, and that's the character's turn: "X finds the body of Y, and raises the alarm!" `[npc.discovers]` plus the faction message if the player is there and not hidden, else `[npc.alarm_nearby]` from next door. A kill a member witnessed marks the body `_found` at once. BURY X (verb `bury`) in a `diggable` room removes a body or a portable thing from play `[bury.done]`; elsewhere `[bury.cant]`.
- **Tracks:** whenever the player or a character leaves a room through an exit (walking, or a character's move), its prints are recorded in that room's private `_tracks` (who, direction, turn; one entry per mover, the latest 8 kept), unless the room is `trackless`, the mover has `leaves_tracks: false`, or `settings.track_life` is 0. Tracks older than `track_life` actions (default 30) are gone. EXAMINE TRACKS (verb `tracks`, `reach: false`) lists them freshest first, merging identical ones: `[tracks.fresh]` (≤ 5 actions), `[tracks.plain]` (≤ 15), `[tracks.old]`, `[tracks.own]` for the player's; `[tracks.none]`, or `[tracks.trackless]` in a trackless room. Prints read `prints` from the character, else its faction, else "footprints". FOLLOW {text} / TRACK {text} (verb `follow_tracks`) takes the freshest non-player tracks whose prints or mover's vocabulary match every word of the text (ignoring "tracks", "trail", "prints"…), says `[tracks.follow]` and walks that way like GO; none → `[tracks.nothing_to_follow]`. A hunter with `tracker: true`, hostile to the player and with no enemy or food to go for, follows the player's tracks in its room that are at most 5 actions old, before going to where it last saw them.
- **Naming:** `nameable: true` on objects and characters. Verb `rename`: name/rename … as {text}, rename … to {text}, call/name/rename/christen {target} {text}, or with no name (a hint, `[name.bad]`, free). Not nameable → `[name.cant]`. The text becomes up to three words of letters, digits, ' and -, each capitalised; anything else → `[name.bad]` (free). Otherwise "From now on, X is called Y." `[name.done]`. The name is the private state field `_name`: it replaces the entity's name everywhere it's shown, makes it a proper name (no article: `a()`, `the()` and lists treat it like the player), and its words match it in the parser alongside its own vocabulary; it's never an "unknown word". Lua: `x.name` is the current name, `x.proper` whether it has been named (both read-only), `x:rename(name)` names it (error if not nameable or unusable) and returns the tidied name.
- **Movement:** characters use exits with a `to` and no `when`, passing doors that aren't locked, blocked or hidden from their side, without opening them. "The X goes north." `[npc.leaves]` when leaving the player's room, "The X arrives." `[npc.arrives]` when entering it.
- **`act`** (Lua, `self` is the character): runs instead of the behaviour. `self:behave()` runs the behaviour from inside it.
- **Hostile behaviours** (hunter, guard) attack enemies: with a `faction`, whoever its `hostile_to` lists (`player` for the player); without one, the player only. Nobody attacks the dead or itself. They use their best carried weapon or their own `damage` (default 1 for them). The compiler warns if one would attack a player who has no `health`.
- **Targets:** a hunter or guard attacks the first enemy in its room; a hunter otherwise steps toward the nearest enemy within `range`. Ties go to the player, then to characters in declaration order. Fights and eating in a room adjoining the player's are heard, once per turn `[npc.fight_nearby, npc.eating_nearby]`; NPC-on-NPC attacks in the player's room are shown `[npc.attacks]`.
- **Self-defense:** `fights_back` defaults to true for every character. `patience` (default 0 for hunter, guard and defender, 2 otherwise) is how many attacks it tolerates; the remaining amount is the `patience` state field. An attack on a character that fights back (the player's, a character's, or a script's `:attack`) uses up 1 patience if it has any ("X is losing patience with you/Y." `[npc.losing_patience_you, npc.losing_patience]`, shown if the player is in the room and it survived), and otherwise adds the attacker's key to its `foes` state list. `foes` and `patience` aren't described and are reserved as source state names. A character treats its foes as enemies while they're alive. Hunters and guards add foes to their enemies as usual. Other behaviours attack a foe in their own room before doing anything else, and never chase. Setting `foes` to `nil` forgives everyone.
- **Hiding:** the player is hidden when shut inside something closed and not transparent (the same rule as "You're shut inside…"). A hidden player isn't a target: guards ignore them, and hunters only remember the room where they last saw the player in range (`_last_seen`). With no enemy or food to go for, a hunter heads there, and on arriving "X searches all around, but finds nothing." `[npc.searches]`, then forgets.
- **Noise:** a noise at a room with a loudness: every active hunter whose shortest route to that room (by `npc_exits`) is at most `loudness` moves, whatever its `range`, takes it as where it last saw the player (`_last_seen`), and goes to investigate as for hiding. Sources: SHOUT (verb `shout`: shout, yell, scream, holler, make noise, call out; "You shout at the top of your lungs." `[shout.done]`) at the player's room with loudness `player.voice` (default 2, stored as `settings.voice`); a timer's `noise`; and Lua `make_noise(place, loudness)` or `x:make_noise(loudness)`, which return how many hunters were drawn.
- **Fire and destruction:** traits `flammable` and `destructible` (alias `destructable`). Verb `burn` (burn {target} (with {tool}), set {target} alight (with {tool}), set {target} on fire, set fire/light to {target}, torch {target}); LIGHT on a flammable non-light-source does the same, and EXTINGUISH puts it out. It needs the target flammable `[burn.cant]`, not already burning `[burn.already]`, and something burning (a lit light source, or a burning thing) held by the player, the named tool if given `[burn.no_flame]`; then `burning` is set, for three turns (`_fire`) `[burn.done]`. Anything burning counts as a burning light (lights dark rooms) and as `fire` for `fears`; a character that fears fire flees a room with a fire in plain sight. Each turn, after characters act, every fire sets one more flammable object alight (inside it first, else in its room in plain sight) `[fire.spreads]`, then burns down; at zero, a destructible thing is destroyed, its contents falling where it was `[fire.burnt_up, destroy.spills]`, and anything else stops burning `[fire.burns_out]`. Messages only for the player's room. Fire is always visible: tags and room sentences show `burning` even for unknown things. Attacking a destructible object with no health smashes it `[smash.done, destroy.spills]`.
  - **Spreading next door:** a fire with nothing to catch in its room sets alight the first flammable object in plain sight in a room through an exit whose door (if any) is open `[fire.spreads_away, fire.spreads_in]`. Spreading happens before burning down, and only fires burning at the start of the turn burn down.
  - **Scorching:** each turn every character (active, with health, not invincible) and the player in a room with a fire loses 1 health `[fire.scorches_you, fire.scorches]`, and sleepers there wake. A fire remembers who started it (`_lit_by`, passed on as it spreads); a character it kills gets `killed_by` that, and its body says `[state.killed_burnt]`.
  - **Smoke:** with a fire in a room next to the player's (and none in theirs), `[fire.smoke]` (with `args.from`). Peeking into a burning room adds `[peek.fire]`.
  - **Putting out:** PUT OUT/EXTINGUISH by hand works only on a `portable` fire `[fire.too_big]`. Verb `douse` (pour/tip {tool} on/over {target}, douse/extinguish/put out/smother {target} with {tool}), or THROW {tool} AT a burning {target}, with a `douses: true` object puts it out and removes the object `[douse.done, douse.cant]`.
- **Traps:** `trap: { holds: N }` (N ≥ 1) on objects; the `armed` state (described like any flag: "(armed)"). Verb `set_trap` (set/arm/lay/set up/prime) moves it to the player's room, armed, as the player's (`_players`) `[trap.set, trap.already, trap.not_trap]`; `disarm_trap` (disarm/defuse/unset) `[trap.disarm, trap.not_set]`; TAKE disarms an armed trap first `[trap.take_armed]`. Whenever a character moves into a room (by its behaviour, or `:go`/`:wander`/`:step_toward`), or the player walks in, the first armed trap lying loose there springs on it, unless it's the player's own trap and the player, or the mover has `leaves_tracks: false`: the trap is disarmed and unhidden, the victim held for `holds` turns (`_held`, `_held_by`; the player one more, as they work free at the end of each turn), and with the object's `damage` hurt (for a character, as an attack by the player with the trap as weapon) `[trap.catches, trap.catches_you]`. A held character's turn: attack an enemy in its room if it can see one, else struggle `[trap.struggles]`, until free `[trap.breaks_free]`; it's tagged "caught" `[state.caught]`, always shown. A held player can't walk `[trap.stuck]` until `[trap.you_free]`.
- **Mimicry:** a noise may carry a `faction`; then only that faction's hunters take it as their last sighting (sleepers in earshot all wake). Factions have `calls:` (sounds, lowercased). Verb `mimic` (imitate/mimic/impersonate {text}, call like {text}, and bare howl/bark/growl/roar/hoot/screech/squawk/caw/grunt/bleat): the sound or text words pick the first faction whose `calls`, key or members' nouns match (plurals ignored), and make a noise at the player's room with loudness `player.voice` for it `[mimic.call, mimic.impression]`; no match: a noise for everyone `[mimic.strange]`. Verb `sound` (blow/sound/play/ring/toot {target}): an object with `calls: { faction?, loudness }` makes that noise where it is `[sound.done]` (`args.verb` is the word used); otherwise `[sound.cant]`. Timers' `noise` and Lua `make_noise(place, loudness, faction?)` / `x:make_noise(loudness, faction?)` take a faction too. Faction names are checked.
- **Disguises:** `disguise: <faction>` on a worn object makes the player present as that faction instead of `player`. Foes (anyone the player has provoked) still attack.
- **Attitude:** a faction's attitude toward the player is kept per presented side, in the player's `_attitude` state map (`"goblins>player"`, `"goblins>goblins"`). It starts at 0 if the faction's `hostile_to` lists that side, else at the faction's `patience` (default 2). A faction member attacks the player exactly when its faction's attitude toward the player's current side is 0 or below (or the player is its foe). When the player becomes a new foe of a member (its own patience used up) and the faction **witnessed** it (some active member, the victim included if it survived, is in the player's room, and the player isn't hidden), that faction's attitude toward the current side drops by one, if above 0. Unwitnessed provocations don't change it, and killing witnesses afterwards doesn't undo it: "The X are growing wary of you." `[faction.wary]`, and at 0, "The X turn against you." `[faction.turns]` or, if disguised as that faction, "The X see through your disguise!" `[faction.sees_through]`. At 0, every member attacks the player on sight in its room, whatever its behaviour (hunters also hunt), except **pacifists**: `pacifist` (default true for merchant and conversationalist, false otherwise; ignored with a warning on hunters and guards) means a character never fights: not with its faction, and not back (`patience` and `fights_back` have no effect, with a warning). An attack still adds the attacker to a pacifist's `foes`, and the first time, lowers its faction's attitude as for any other member.
- **Apologies:** APOLOGIZE (verb `apologize`: apologize/apologise to, say sorry to, make peace with, beg/ask … for forgiveness, calm (down); `reach: false`; takes a turn). Outcomes, in order: not an active character → `[apologize.not_person]`; hostile to the player's current side by nature (a faction whose starting attitude toward that side is 0, or a hunter/guard without a faction) → `[apologize.refused]`; no grudge, full patience, and its faction's attitude at its start → `[apologize.nothing]` (doesn't count); already accepted `forgiveness` apologies (default 1, tracked in `_forgiven`) → `[apologize.exhausted]`; otherwise `[apologize.accepted]`: the player leaves its `foes`, its `patience` resets to the starting value, and if its faction's attitude toward the current side is below its start, it goes up by 1 `[faction.calmer]`. Attitude never recovers on its own.
- **Gifts:** GIVE X TO Y (verb `give`: give/hand/offer/feed {target} to {tool}). X must be held directly `[give.not_held]`; Y an active character `[give.not_person]`; no Y → `[give.whom]` (free). If X is in Y's `tame_with`, Y is **tamed** (`_tamed`) `[give.tamed]`, or `[give.tamed_eaten]` when X is food for Y's faction (`eaten_by`), which is then removed; otherwise X moves to Y. If X is in Y's `likes`, Y is pacified as by an apology, without using `forgiveness` `[give.liked]`, and `[faction.calmer]` if its faction warmed. Otherwise `[give.unwanted]` and nothing moves. Taming pacifies too.
- **Tamed characters** never attack the player; treat as enemies anyone hostile to the player; on their turn attack the first such enemy in their room, else step toward the player if apart (whatever their `range`), else stay. Their faction's enemies still treat them as members.
- **Bribes:** verb `bribe`: give N coin(s)/money to {target}, pay N to {target}, bribe {target} (with N). No number → `[bribe.how_much]` (free). In order: not an active character; no `bribe` on it `[bribe.refused]`; already bribed `[bribe.already]`; more than the player has `[bribe.cant_afford]`; less than `bribe` `[bribe.too_little]` (nothing paid); else the money moves (to the character if it has `money`), it's pacified, and **bribed** (`_bribed`) `[bribe.taken]`: it never attacks the player.
- **Throwing:** verb `throw`: throw/hurl/toss/chuck {target} at {tool}; throw/toss/hurl {target} {direction}; throw/toss/hurl {target}. X must be held directly `[throw.not_held]` and weigh at most 3 `[throw.too_heavy]`. AT Y: X lands in the player's room `[throw.at]`; if Y is alive and has health or is invincible, it's an attack with X as the weapon (narrated as any attack, recording `killed_with`); otherwise `[throw.bounces]`. In a direction: the first exit that way with a `to` (ignoring `when`, but not hidden), unless through a closed door `[throw.door]`; none → `[throw.no_way]` (free). X lands in that room `[throw.direction]`, with a noise of loudness 1 there. Otherwise `[throw.away]`. Then every active character with `fetches` that's a follower or tamed, not already fetching, and able to reach X sets off `[npc.fetches]` (if with the player).
- **Fetching** (`_fetching`): before following, a fetcher goes to X, grabs it if it's lying loose in its room `[npc.grabs]`, goes back to the player and drops it in the player's room `[npc.brings]`. It gives up if X isn't lying in a room or it can't get there; carrying X with no way back, it drops X where it is.
- **Undoing:** when the player becomes a new foe of a tamed or bribed character, it stops being either.
- **Invincible:** `invincible: true` on a character means attacks do no damage, with "X shrugs off the blow." `[attack.invincible]` after the player's attack. It can be attacked even without `health`, and attacks on it still use up its patience and count toward faction attitude. The compiler checks the faction, and warns if the object isn't `wearable`.
- **Bait:** `eaten_by: [<faction>…]` on objects. A member of one of those factions with food lying loose in its room eats it (one bite a turn; it lasts `size` bites, tracked in `_bites`): "X tears into Y." / "X finishes off Y." `[npc.feeds, npc.finishes]`, heard next door like eating corpses. Food comes before everything except fighting back against a foe in the room. A hunter heads for the nearest food in range when it's no farther than the nearest enemy.
- **Corpse stories:** the blow that takes a character to 0 health records `killed_by` (the attacker's key) and `killed_with` (the weapon's key, if any) in its state. Examining the body adds a sentence: `kills` from the weapon, else the killer, else the killer's faction, in "X has been <kills>." `[state.killed_how]`; with none, "X was killed by Y." `[state.killed_by]` or "You killed X." `[state.killed_by_you]`. Authors can set these fields themselves, with `dead: true`, to start a character dead.
- **Factions:** top-level `factions:` maps IDs to `hostile_to` (other faction IDs, or `player`), `eats_corpses`, `kills` and `patience`. Hostility is one-way. `player` can't be a faction ID. Characters join one with `faction:`; unknown IDs are errors with suggestions.
- **Corpses:** unless `corpse: false`, a defeated character stays where it fell with the state `dead: true` (tag "dead", always shown; "X is dead." `[state.dead]`). The dead don't act, can't be attacked `[attack.dead]` or talked to `[talk.dead]`, and match the lexicon's `corpse_words` (corpse, body, remains, carcass, bones). A member of an `eats_corpses` faction with no enemy in its room eats a body there instead of anything else (it's removed; "X devours what's left of Y." `[npc.eats]`); a hunter with nothing to hunt heads for the nearest body in range.

**Health and damage.** `health` on characters and the player is the maximum, and the current value is the `health` state field (reserved, like `money`). `damage` on characters and the player is unarmed damage (player default 1); on objects it makes a weapon. An attack takes the weapon's damage, else the attacker's, else 1, never going below 0 health.

- **ATTACK** (verb `hit`: hit, attack, kill, fight, strike, smash, stab, slay, punch, break, optionally WITH a held weapon): needs reach. "You hit X (with Y)." `[attack.done]`; no health → `[attack.pointless]`, which, like a script verb, gives way to `revealed_by`'s "You find …"; a tool without `damage` → `[attack.not_weapon]`; not held → `[attack.not_held]`. Handlers on `hit` still replace it per object.
- **Wounds:** below full, "wounded"; at half or less, "badly wounded" `[state.wounded, state.badly_wounded, state.tag.*, state.here.*]`. Not shown for the player.
- **Defeat:** checked after the player's action (before characters act, so a killing blow isn't answered), after each character's turn, and after timers. The character's `defeated` handlers run (`self` is it). If one healed it or moved it out of play, that's all. Otherwise, unless one returned `block()`, "X is defeated." `[npc.defeated]` and it drops what it carries `[npc.drops]`. Then it becomes a corpse, or with `corpse: false` leaves play (location `nil`).
- **Death:** at 0 player health, global `died` handlers run. If none ended the game or restored health, the game ends with `died`. The compiler adds a loss ending `died` ("You have died." `[ending.died]`) when the player has `health` and the author didn't write one.
- **HEALTH / DIAGNOSE / HP / STATUS:** `[health.full, health.hurt, health.none]`. Doesn't take a turn.

**Trading.** `price` on objects; `money` on the player and optionally merchants (state field; without it a merchant can always pay).

- **WARES** (browse, shop, trade (with X)): "X has for sale: a lamp (8 coins) …" `[wares.list, wares.item, wares.empty]`. Takes a turn.
- **BUY X (FROM Y):** X must be priced and carried by a merchant in the room; the merchants' priced goods are in scope for BUY and EXAMINE. Checks money `[buy.too_poor]` and carrying capacity (take messages). `[buy.done]`
- **SELL X (TO Y):** X must be held directly. The offer is `price × buys_at / 100` (`buys_at` default 50, 0–100); 0 → `[sell.worthless]`. A merchant with `money` can't pay more than they have `[sell.merchant_poor]`. Without TO, the first merchant here. `[sell.done]`
- **OFFER N FOR X** (verb `offer`: offer/bid {text} for {target}; haggle/bargain for {target}; `reach: false`; X in scope as for BUY): the first number in the text is the offer; none → `[offer.how_much]`. More than the player has → `[offer.cant_afford]`. At or above the price, it's a BUY at the price. Otherwise, with the merchant's `haggle` (percent, 0–100, default 25 for merchants) at 0 → `[offer.fixed]`; at or above the floor `ceil(price × (100 − haggle) / 100)` → bought at the offer `[offer.accepted]`; below it, if the merchant has `patience` left, it drops by 1 and the merchant counters with the floor, which ACCEPT takes `[offer.counter]`; with none left, the merchant refuses to trade (`_refuses`) `[offer.insulted]`.
- **TRADE X FOR Y** (verb `trade`: trade/swap/exchange/barter {target} for {tool}; Y in scope as for BUY): X must be carried `[sell.not_held]`; Y must be for sale here (as BUY). X's worth is `price × buys_at / 100` (0 → `[sell.worthless]`). At or above Y's floor `ceil(price × (100 − haggle) / 100)`, X goes to the merchant and Y to the player (if it fits, else X comes back), with no money `[trade.done]`. Below it: with `haggle` 0, just `[trade.more]`; otherwise patience drops by 1 `[trade.more]`, or with none left the merchant refuses to trade `[offer.insulted]`.
- **SELL X (TO M) (FOR N)** (also ask/demand N for X). With `haggle` 0, it sells at once at the offer `price × buys_at / 100` `[sell.done]` (asking more → `[sell.fixed]`). Otherwise: with no amount, a deal already on the table for X is taken `[sell.done]`, else the merchant puts its offer on the table `[sell.offer]` (`args.example` is a suggested higher ask). With an amount: at or below the ceiling `min(floor(offer × (100 + haggle) / 100), max(price, offer), merchant's money if it has any)` it sells at the amount (`[sell.accepted]` if above the offer); above it, with patience left, patience drops by 1 and the ceiling goes on the table `[sell.counter]`; with none, `[offer.insulted]` and the merchant refuses to trade.
- **Deals on the table:** a merchant's last offer or counter-offer, buying or selling, is kept in its `_deal` state until it's taken or replaced. ACCEPT (accept (the) offer/deal, deal, agree, shake on it) takes the first deal on the table among merchants here (`[buy.done]` / `[sell.done]`), or `[accept.nothing]`. When buying, the counter-offer is the merchant's floor.
- **Refusing to trade:** a merchant with `_refuses` answers BUY, SELL, OFFER and WARES with `[trade.refused]`. An accepted apology clears it (and restores patience).
- **Money text:** `money.amount` ("8 coins", "1 coin"). INVENTORY ends with "You have 12 coins." `[money.have]` when the player has money.

## Choices

A choice graph has a `start` node and `nodes`. Each node has `text` and `options`. Each option can have:

- `text`: what the player sees.
- `when`: a condition. The option is hidden while it is false.
- `say`: text to print when chosen.
- `do`: Lua to run.
- `goto`: the next node.
- `end: true`: closes the graph.

A node can also have `do` and `goto` without options, which makes it a pass-through.
While a choice is open, input switches from the parser to numbered options. The web build can show these options as buttons.

## Never leave the player stuck

The goal is that a player should never feel the game doesn't understand
what they meant. To get there, the parser works through a fallback chain.
It stops at the first step that produces a valid action. Steps marked
*(planned)* are not built yet. See **Parser behavior** below for what
players actually see.

1. **Exact match** against the game's vocabulary: author words plus the standard library.
2. **Normalization.** Inflected forms are reduced to their base word (`grabbed` → `grab`, `keys` → `key`). Filler like "please" and "I'd like to" is dropped, and pronouns (`it`, `them`) are resolved.
3. **Built-in synonym lexicon.** The standard library maps synonym phrases to verbs, e.g. `snatch` → `take` and `peer at` → `examine`. It grows with each release. v1 has about 27 verb phrases; noun synonyms come only from authors for now.
4. **Words from descriptions.** If no name matches, a word the player read in a description can refer to what it describes. Every word must be in a thing's name, synonyms or description (any form: `glinting` matches "glints"), and at least one must come from the description. Things in scope are tried first; then, if every word is in the current room's description, the reply is "That's just part of the scenery." `[parser.scenery]`. Both branches of `${if}` count, expressions don't, and the lexicon's `stopwords` (plus articles, pronouns and directions) and words under three letters are skipped. A word found in more than one description gets a clarifying question.
5. **Typo correction.** Edit distance is checked against the words this game actually uses, never the whole dictionary, so corrections stay relevant. A confident correction is applied silently. If two words are equally close, nothing is corrected.
6. **Clarifying question**, when the input matches more than one thing ("Which do you mean, the brass key or the iron key?") or is missing a part ("What do you want to fill the great lamp with?"). The player's next input is merged back into the command. *(Planned: "Did you mean …?" for near misses that are too uncertain to correct silently.)*
7. **Author hook:** `on.parse_failed` gets the raw input, so an author can handle unexpected attempts with custom responses.
8. **AI fallback** *(planned; optional, off by default, hosted web only)*. The author turns it on per game with `game.ai_fallback: true`. The engine sends the input plus a list of actions that are valid right now, i.e. what's in scope and which verbs apply. The model must pick one of those actions or answer "none". It cannot write prose or change state. Its pick then runs through the normal action pipeline. Responses are rate-limited and cached. Offline and desktop builds skip this step.
9. **Helpful failure.** The game never prints a bare "I don't understand." It names the part of the input it did understand and suggests verbs that apply here.

**Synonyms are generated when the game is built, not while it runs** *(planned)*. For every
author noun and verb, the compiler suggests extra synonyms. The web editor
shows them so the author can accept or reject each one. Accepted synonyms
are saved into the bundle. At runtime the game is therefore deterministic,
works offline, and behaves the same in the web and desktop builds.

**Tests for the parser** *(planned)*. Each standard library release ships with a corpus of
real player phrasings, together with the action each one should map to.
Changes to the parser are checked against that corpus, so a regression
shows up in CI. For now, the examples in **Parser behavior** are covered by
end-to-end tests in `crates/eifgb-lua/tests/play.rs`.

**Learning from players** *(later, opt-in)*. Hosted web games can report
inputs that failed to parse, grouped together, e.g. "14 players tried
*climb rocks* on Rocky Shore". The author sees these in the editor.

## Parser behavior

This is what players see, using the lighthouse game unless noted. Each
reply comes from a standard message (key in brackets), so authors can
reword any of them under `messages:`.

### Understood without fuss

| Player types | What happens | Why |
|---|---|---|
| `serch pool`, `examin brass kye`, `nrth` | Runs as `search pool`, `examine brass key`, `north` | Typo correction (step 5), using only words this game uses |
| `x glinting`, `search the glint` | Examines / searches the tide pool | The pool's description says "Something glints at the bottom" (step 4) |
| `x waves`, `take black rocks` | "That's just part of the scenery." `[parser.scenery]` | The words are only in the room description (step 4) |
| `grabbed the can` | Runs as `take the can` | Stemming (`grabbed` → `grab`), then the synonym `grab` → `take` |
| `peer at keys` | Examines the brass key | Synonym phrase, plus the plural `keys` → `key` |
| `please drop the can`, `i'd like to snatch the kerosene` | Drop / take the oil can | Filler dropped; `kerosene` is an author synonym |
| `pool` | Examines the tide pool | A bare noun means EXAMINE |
| `x key` then `drop it` | Drops the key | `it` is the last thing mentioned |
| `take all`, `drop everything` | `cup: Taken.` / `plate: Taken.` … | ALL works with TAKE and DROP |
| `unlock gate` | "You unlock the iron gate with the brass key." | The right key is used if you're carrying it |
| `up` (gate unlocked but shut) | "(first opening the iron gate)", then the move | Unlocked doors open on the way through |
| `n` (two doors lead north) | "Which do you mean, the red door or the blue door?" Answering `blue` goes through it | Several exits the same way, told apart by their doors |
| `go through red door`, `enter red door` | Leaves by the exit that uses that door | |
| `search pool. n then take can` | Runs all three | `.`, `;` and `then` chain commands; the chain stops at the first failure |
| `again` / `g` | Repeats the last command | |

### Clarifying questions

The player's next input answers the question. If it doesn't fit the
question, it's treated as a new command.

| Player types | Reply | Then |
|---|---|---|
| `take key` (two keys here) | "Which do you mean, the brass key or the iron key?" `[parser.ambiguous]` | `iron` takes the iron key |
| `x` | "What do you want to examine?" `[parser.need_target]` | `pool` examines it |
| `fill lamp` | "What do you want to fill the great lamp with?" `[parser.need_tool]` | `oil` fills it |

Before asking which thing is meant, the parser narrows the matches by what
makes sense for the verb. TAKE prefers things you aren't holding. DROP, PUT,
WEAR and EAT prefer things you are holding. TAKE OFF prefers things you're
wearing.

### When it can't help

The game never prints a bare "I don't understand." The author's
`parse_failed` handler runs first. If it prints anything, that is the
reply, e.g. `swim` → "The waves would smash you against the rocks."
Otherwise:

| Player types | Reply |
|---|---|
| `dance` | "I don't know the verb "dance". Things to try here: SEARCH, EXAMINE, TAKE, LOOK or INVENTORY." `[parser.unknown_verb]` The suggestions are verbs that nearby objects respond to, plus basics. |
| `take zorkmid` | "I don't know the word "zorkmid"." `[parser.unknown_word]` |
| `take radio` (radio is elsewhere) | "You can't see any such thing." `[parser.not_here]` Also for a word from another room's description, such as `x staircase` on the shore. |
| `take key from` | "I only understood you as far as wanting to take." `[parser.partial]` |
| `take it` (nothing mentioned yet) | "I'm not sure what you're referring to." `[parser.no_pronoun]` |
| `open all` | "You can't use "all" with that." `[parser.no_all]` |
| *(blank)* | "Pardon?" `[parser.empty]` |

A failed parse doesn't take a turn and can't be undone, because nothing happened.

### Inside a choice

While a menu or dialogue is open, input picks an option instead of being
parsed. The player can type the option's number or part of its text
(`goodbye`). Anything else gets "Please choose one of the numbered
options." `[choice.invalid]`, and the options are shown again. UNDO,
SAVE, RESTORE, RESTART and QUIT still work.

### Known gaps

- PUT and WEAR don't pick the thing up for you: "You need to be holding the wooden box first." `[put.not_held]`.
- There's no setting for proper names yet, so article helpers would say "the Bob".

## Standard library versioning

- Standard verbs, the synonym lexicon and default messages are a versioned package, and `game.stdlib` pins the version a game uses.
- Rebuilding an old game with a newer engine keeps the pinned version, so its behaviour stays the same.
- An author can upgrade explicitly. When they do, the compiler reports what changed, e.g. "new verb `climb` now responds in 3 rooms where it used to fail".
- When a standard verb and an author verb share a name or pattern, the **author always wins**, and the compiler shows a warning.

## Localization readiness

Only English ships for now, but the engine is built so that other languages can be added later without a rewrite:

- **No English text inside the Rust engine.** Every engine message comes from a message catalog, keyed by ID (e.g. `msg.take.already_held`). The catalogs are part of the standard library.
- **Parser grammar is a separate module for each language.** Tokenizing, articles, pronouns, word order and normalization rules live in a module chosen by `game.language`. Only `en` exists to begin with.
- **Text fields can become per-language later without breaking files.** Today, prose fields are plain strings, meaning "the game's language". A later format version will also accept per-language maps such as `name: { en: brass key, fr: clé en laiton }`, so existing files stay valid.
- **Translatable text is easy to find.** The compiler can pull every prose string out into a translation file.
- **Templates and Lua must stay language-neutral.** Rules about grammar, such as articles and plurals, belong in engine helpers like `${the(self)}` and `${a(obj)}`. Scripts shouldn't build sentences by concatenating strings.

## Open questions

1. Template syntax: is `${...}` acceptable? It clashes with nothing in Lua or YAML, but it's noisy.
2. Should `characters` be separate, or just objects with a `character` trait?
3. Verb grammar: is `{target}`/`{tool}`/`{text}` enough, or do we want named slots?
4. Should handlers be able to take over the parser entirely (raw input hooks)?
5. Localization: should the parser and default messages be swappable per language from the start?
