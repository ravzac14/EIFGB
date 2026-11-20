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

## Players

- **WHERE IS THE KEY / FIND THE KEEPER:** where the player last saw it
  ("You last saw the keeper in the lamp room"), using what the game
  already remembers; GO TO THE KEEPER walks there.
- **UNDO 3:** take back several turns at once.
- **OOPS and "Did you mean…?":** OOPS LANTERN fixes the misspelled word
  in the last command, and near misses too uncertain to fix silently ask.
- **VERBOSE / BRIEF:** describe rooms in full on every visit, or only the
  first time.
- **RECAP:** what's happened so far and where things stand, for a player
  coming back to a game after a week.
- **ASK ABOUT goes to the topic:** ASK KEEPER ABOUT THE GATE opens the
  conversation at the option that mentions the gate.
- **Commands that remember:** TAKE ALL THEN GO NORTH works; extend it to
  AGAIN with a new object ("again, with the iron key").

## The editor

- **A picture library:** reuse a picture across rooms and things, and see
  how much space a game's pictures take in the browser.
- **Find in game:** search every room, thing and script for a word.
- **Map tidy-up:** lay out rooms neatly by their directions, and spot
  exits that cross or overlap.
- **Undo history:** a list of recent changes to step back to, not only one
  at a time.
- **Playtest from a save:** start test play from any point a recording
  reached, without replaying from the start.
- **Hints from the puzzle chain:** draft HINT steps from the Puzzles tab
  ("Search the worn mat…").
- **Timer timeline:** when each timer can go off, and what it does, in
  turns and (with a clock) hours.

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
