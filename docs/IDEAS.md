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
- **Fire and smashing:** `flammable`, `destructible`, SET … ALIGHT, SMASH.

## The world

### More fire

Fire is built (spreading in a room, burning away, lighting rooms,
scaring fire-fearers). Still to do: fire spreading through doorways to
the next room, burning characters (damage in a burning room), smoke
that can be seen and smelt from nearby, and water or sand to put fires
out.

- Builds on: fire, noise, darkness.
- Effort: medium.

### Barter

TRADE LAMP FOR ROPE with a merchant, using item prices and haggling
patience instead of coins.

- Builds on: prices, haggling, patience.
- Shape: TRADE X FOR Y: accepted when X's resale value covers Y's price
  less the merchant's haggle; counters ask for "something more".
- Effort: small.

### Pushing and climbing

PUSH CRATE NORTH to drag heavy things between rooms, STAND ON CRATE to
reach a high shelf.

- Builds on: weight, supporters, enterable things.
- Shape: PUSH X <direction> for heavy `pushable` things; `high: true`
  things out of reach unless the player is on something.
- Effort: medium.
