# Ideas for later

Things a creative player might try, that we'd like the engine to support.
Each builds on something that already exists.

Everything from the first list has now been built:

- **Disguises, bait, corpse stories and hiding:** see "Tricks players can
  find" in the README, and the advanced tutorial.
- **Making noise:** SHOUT, timer noises, `make_noise`.
- **Calming down:** apologies, which are manual and limited, and also
  restore a faction's attitude.
- **Haggling:** OFFER … FOR when buying, SELL … FOR when selling, and ACCEPT.
- **Taming and gifts:** `tame_with`, `likes` and `bribe`.
- **Throwing:** THROW … AT / THROW … NORTH, and companions that fetch.
- **Naming things:** CALL THE DOG REX, with `nameable`.
- **Tracks:** EXAMINE/FOLLOW TRACKS, `prints`, `trackless` rooms, and
  trackers (from the second list).
- **Witnesses:** a faction only sours when a member sees it.
- **Hiding bodies:** found bodies raise the alarm; carry, hide or BURY them.
- **Sleep and sneaking:** `asleep`, waking, SNEAK, and sneak attacks.
- **Peeking:** PEEK THROUGH a door or PEEK NORTH: an impression of the next
  room, not its description.
- **Darkness:** `dark` rooms, LIGHT and EXTINGUISH, and seeing only what
  you hold in the dark.
- **Pickpocketing:** STEAL/PICKPOCKET/ROB, from sleepers, the distracted
  and the fooled.
- **Eavesdropping:** `chatter`, loud lines through walls, LISTEN AT THE DOOR.
- **Giving orders:** TELL REX TO ATTACK/STAY/FOLLOW/GO/FETCH/DROP, and "REX, …".
- **Fear and morale:** `morale`, `fears`, BRANDISH, fleeing and cornering.
- **Mimicry:** faction `calls`, HOWL/IMITATE, horns that call one faction.
- **Traps:** `trap`, SET/DISARM, hidden traps that catch the player.
- **Fire and smashing:** `flammable`, `destructible`, SET … ALIGHT, SMASH,
  spreading next door, scorching, smoke, and `douses`.

## The world

### Barter

TRADE LAMP FOR ROPE with a merchant, using item prices and haggling
patience instead of coins.

- Builds on: prices, haggling, patience.
- Shape: TRADE X FOR Y: accepted when X's resale value covers Y's price
  less the merchant's haggle; counters ask for "something more".
- Effort: small.

## The editor

### Resizable rooms

Let authors resize a room's box on the editor's map, so a great hall can
look bigger than a closet. Only for picturing the map: it changes nothing
in the game.

- Builds on: the map (`web/src/editor/map.ts`) and `map.yaml`, which
  already keeps positions outside the game file.
- Shape: drag handles on a selected room's corners; store the size beside
  its position in `map.yaml` (`kitchen: [x, y, w, h]`); lines, handles and
  drop targets use each room's own size.
- Effort: small.

### More editor ideas

- **Play inside the editor:** a play panel beside the map, so testing
  doesn't mean switching tabs; optionally "start from this room".
- **Colours for rooms or regions** on the map (with resizable rooms above).
- **Starter templates:** "a small house", "a dungeon with a locked door",
  to start a new game from.
- **A blueprint theme** for the map: white lines on blue, so map work feels
  different from playing.

## The player

### Themes chosen by the game

An author sets the look of their game, which players can still override:
`presentation: { theme: terminal, accent: "#c33" }` in `game.yaml`.

- Builds on: the player's themes and settings.
- Shape: a new optional `presentation` section (web-only hints the engine
  ignores); the player applies it unless the player picked a theme
  themselves. Needs a decision on putting presentation in the game file.
- Effort: small.

### Smaller ideas

- **Parchment theme:** paper texture, and a drop capital on each room's
  first line.
- **Typewriter reveal:** text appears letter by letter; off by default,
  any key skips.
- **Endings collected:** "You've found 2 of 3 endings" on the home page
  card, as a reason to replay.
- **Download a transcript** of a play-through.

## The website

- **A featured game** at the top of the gallery.
