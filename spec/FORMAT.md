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
| `settings`   | Game-wide switches. For now just `describe_state` (default `true`). |
| `vars`       | Global story state (scalars, lists, maps). |
| `player`     | Player description and carrying capacity. |
| `rooms`      | Locations and their exits. |
| `objects`    | Things. Each starts somewhere via `in:`. |
| `characters` | NPCs: objects that can be talked to. |
| `verbs`      | Custom verbs and their grammar patterns. |
| `choices`    | Choice graphs, used for menus and dialogue. |
| `on`         | Global event handlers (`turn`, `start`, ...). |
| `endings`    | Named endings with a `kind` (`win`, `loss` or `neutral`). |

IDs are `snake_case` and unique across rooms, objects, characters and choices.

## Entities

- **Exits** take one of three forms. A room ID is a plain exit. A string is a blocked exit, with that string as the message. A map `{to, through, when, blocked}` handles doors and conditional exits.
  A direction can also take a **list** of those forms, for several exits the same way. When the player goes that way, entries with `to` whose `when` holds (or that have no `when`) are candidates:
  - **One candidate:** the player goes there.
  - **Several, each through a different door:** the game asks "Which do you mean, the red door or the blue door?". The reply runs as GO THROUGH that door, and asking doesn't take a turn.
  - **Several without distinct doors:** the first one wins, and the compiler warns that the rest can't be reached.
  - **None:** the first `blocked` message in the list is shown, or a default.

  So a conditional route is `[{to: bridge, when: vars.bridge_down}, {blocked: "The bridge is raised."}]`. GO THROUGH, WALK THROUGH, ENTER and GO INTO a door also leave by the exit that uses it.
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
  | `fueled: true` | "It's fueled." | "a lamp (fueled)" | "The great lamp is fueled." |
  | `fueled: false` | "It isn't fueled." | *(none)* | *(none)* |
  | `channel: 16`, `color: red` | "Its channel is 16." | *(none)* | *(none)* |

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
- **Methods on every entity:** `:move_to(place)`, `:is_in(holder)`, `:contents()`, `:has(trait)`.
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
4. **Typo correction.** Edit distance is checked against the words this game actually uses, never the whole dictionary, so corrections stay relevant. A confident correction is applied silently. If two words are equally close, nothing is corrected.
5. **Clarifying question**, when the input matches more than one thing ("Which do you mean, the brass key or the iron key?") or is missing a part ("What do you want to fill the great lamp with?"). The player's next input is merged back into the command. *(Planned: "Did you mean …?" for near misses that are too uncertain to correct silently.)*
6. **Author hook:** `on.parse_failed` gets the raw input, so an author can handle unexpected attempts with custom responses.
7. **AI fallback** *(planned; optional, off by default, hosted web only)*. The author turns it on per game with `game.ai_fallback: true`. The engine sends the input plus a list of actions that are valid right now, i.e. what's in scope and which verbs apply. The model must pick one of those actions or answer "none". It cannot write prose or change state. Its pick then runs through the normal action pipeline. Responses are rate-limited and cached. Offline and desktop builds skip this step.
8. **Helpful failure.** The game never prints a bare "I don't understand." It names the part of the input it did understand and suggests verbs that apply here.

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
| `serch pool`, `examin brass kye`, `nrth` | Runs as `search pool`, `examine brass key`, `north` | Typo correction (step 4), using only words this game uses |
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
| `take radio` (radio is elsewhere) | "You can't see any such thing." `[parser.not_here]` |
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
