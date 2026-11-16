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
It stops at the first step that produces a valid action:

1. **Exact match** against the game's vocabulary: author words plus the standard library.
2. **Normalization.** Inflected forms are reduced to their base word (`grabbed` → `grab`, `lamps` → `lamp`). Filler like "please" and "try to" is dropped, and pronouns (`it`, `them`) are resolved.
3. **Built-in synonym lexicon.** The engine ships a large, curated English synonym map for verbs and common nouns, e.g. `snatch` → `take` and `peer at` → `examine`. It is part of the standard library, so it grows with each release.
4. **Typo correction.** Edit distance is checked against the words this game actually uses, never the whole dictionary, so corrections stay relevant.
5. **Clarifying question**, when the input matches more than one thing ("Which do you mean, the brass key or the iron key?"), or when it is a near miss ("Did you mean *fill the lamp*?").
6. **Author hook:** `on.parse_failed` gets the raw input, so an author can handle unexpected attempts with custom responses.
7. **AI fallback** *(optional, off by default, hosted web only)*. The author turns it on per game with `game.ai_fallback: true`. The engine sends the input plus a list of actions that are valid right now, i.e. what's in scope and which verbs apply. The model must pick one of those actions or answer "none". It cannot write prose or change state. Its pick then runs through the normal action pipeline. Responses are rate-limited and cached. Offline and desktop builds skip this step.
8. **Helpful failure.** The game never prints a bare "I don't understand." It names the part of the input it did understand and suggests verbs that apply here.

**Synonyms are generated when the game is built, not while it runs.** For every
author noun and verb, the compiler suggests extra synonyms. The web editor
shows them so the author can accept or reject each one. Accepted synonyms
are saved into the bundle. At runtime the game is therefore deterministic,
works offline, and behaves the same in the web and desktop builds.

**Tests for the parser.** Each standard library release ships with a corpus of
real player phrasings, together with the action each one should map to.
Changes to the parser are checked against that corpus, so a regression
shows up in CI.

**Learning from players** *(later, opt-in)*. Hosted web games can report
inputs that failed to parse, grouped together, e.g. "14 players tried
*climb rocks* on Rocky Shore". The author sees these in the editor.

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
