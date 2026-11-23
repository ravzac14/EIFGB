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

The player commands from the next list are built too: WHERE IS and GO TO
a thing, UNDO 3, OOPS and "Did you mean…?", BRIEF and VERBOSE, RECAP, ASK
ABOUT going to the topic, and AGAIN WITH (see the command reference in the
README).

So are the editor ideas: a picture library, Find in game, map tidy-up,
undo history, test play from a step of the walkthrough, hints drafted from
the puzzle chain, and the timer timeline (see "Step 7" in the README).

## The puzzle chain

Steps solved inside Lua are marked, and what the chain takes on trust is
listed, but it can't see what those scripts need. Two ways to close the
gap:

- **Authors describe a scripted step:** an optional `step:` on a handler,
  e.g. `step: { text: Give the bread to the child, needs: [bread] }`, that
  the chain uses as written. Precise, but something more to keep in step
  with the script.
- **Learn from the walkthrough:** the editor already replays it after
  every change. It could note what each command made possible ("command
  7, GIVE BREAD TO CHILD: the coin appeared, the gate opened") and use
  that wherever reading the game falls short. True to how the game plays,
  but only for what the walkthrough does.

## The player

- **Bookmarks in the story:** mark a moment and jump back to read it.
- **Achievements:** optional `achievements:` a game can award, shown
  with the endings found.

## The website

- **Ratings and play counts** for gallery games, kept privately in each
  browser first.
- **A "Game jam" page:** a theme, a deadline, and the entries.
- **Translations:** the player's and editor's own words in other
  languages, starting from the stdlib's language files.
