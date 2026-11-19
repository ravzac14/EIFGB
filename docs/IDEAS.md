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

- **ASK and TELL about topics:** ASK GUARD ABOUT KEY, from `topics:` on
  characters, alongside the numbered conversations.

## The editor

- **A puzzle chain view:** what unlocks what (key → door → room →
  ending), flagging endings nobody can reach and keys that open nothing.
- **Rename everywhere:** change an id and every reference follows: exits,
  `unlocks_with`, scripts and `map.yaml`.
- **"What did players try?":** test play keeps the commands the game
  didn't understand, and suggests objects or synonyms to add.

## The player

- **Read aloud:** speak new text with the browser's own voices.
- **Room pictures:** an optional `picture:` per room, shown above its
  description and in the map panel.

## The website

- **A featured game** at the top of the gallery.
