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

## Manipulating creatures

### Giving orders

TELL REX TO ATTACK THE GOBLIN, REX, STAY, REX, GUARD THE DOOR, REX, GO
NORTH.

- Builds on: taming, followers, character methods (`step_toward`,
  `attack`, `go`).
- Shape: an ORDER/TELL verb and "NAME, command" syntax for tamed
  companions; a small set of orders (attack, stay, follow, go, fetch,
  guard) stored as state and used by the tamed behaviour.
- Effort: medium.

### Fear and morale

Characters flee when badly hurt or outnumbered, or when the player waves
something they fear: a torch at wolves, the chief's head at goblins.

- Builds on: behaviours, factions, the alpha's flee script.
- Shape: `fears: [torch]` on factions or characters, `morale` thresholds
  (health fraction, outnumbered); fleeing = stepping away from the threat;
  WAVE/BRANDISH X verbs.
- Effort: medium.

### Mimicry

HOWL draws only wolves; BLOW GOBLIN HORN draws only goblins. A noise only
one faction answers.

- Builds on: noise, factions.
- Shape: noises with an optional `faction:` (timers, `make_noise`, and a
  `calls: wolves` field on objects or verbs).
- Effort: small.

### Traps

SET SNARE in a room. A hunter walking through is caught for a few turns.
Bait it to choose who you catch.

- Builds on: pathfinding, bait, timers.
- Shape: `trap: { holds: 3, damage: 1 }` on objects; set in a room, it
  catches the first character to enter (or the player, if careless).
- Effort: medium.

## The world

### Fire

LIGHT THE TENTS: fire spreads to `flammable` things over a few turns,
scatters creatures that fear it, and can be heard and seen from nearby.

- Builds on: timers, state, noise, fear.
- Shape: a `burning` state that spreads each turn to flammable things in
  the room and next door; burns out after N turns; damages characters in
  burning rooms.
- Effort: large.

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
