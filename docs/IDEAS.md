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

The editor ideas are built too: a picture library, Find in game, map
tidy-up, undo history, test play from a step of the walkthrough, hints
drafted from the puzzle chain, and the timer timeline (see "Step 7" in the
README); and so are bookmarks in the web player.

So are WHAT DID SHE SAY?, MAP in words, and where things came from
(WHERE IS on something carried, INVENTORY BY PLACE); counting (TAKE TWO
COINS, TAKE ALL BUT THE KEY) was already there.

## Writing games

- **Text that changes with visits:** `first:` and `later:` descriptions,
  or `${visits}`, without counting by hand in a `_visits` field.
- **Variety:** a description or reply as a list, `cycle:` or `random:`, so
  WAIT and a ticking clock don't say the same thing every time.
- **Daily routines:** characters who keep to a clock (`routine: { "6:00":
  bakery, "13:00": market }`), and are found where they should be.
- **How people feel about you:** a `regard` that gifts, answers and
  insults move, which conversation options and prices can check.
- **Scenes:** named stretches of the story, with a start and an end
  (`when:`), shown in RECAP and the journal, and usable in conditions.
- **Locked options shown:** a conversation option that's not yet
  available shown greyed, with why ("You'd need something to trade").
- **Games in more than one language:** a game's own text in a second
  language, chosen by the player.

## The editor

- **Conversations as a graph:** a choice's nodes and options drawn like
  the map, to drag and connect.
- **Words in descriptions:** nouns a description mentions that aren't a
  thing or scenery yet, each one click from being one.
- **Spelling:** a spell check of the game's own text, with its names
  allowed.
- **Walkthrough coverage:** rooms, things and conversation options the
  walkthrough never reaches.
- **A walkthrough for each ending,** all checked after every change.
- **A wanderer test:** a test player that tries things at random for a
  few thousand moves, and reports errors, dead ends and places it got
  stuck.
- **Expected replies:** a walkthrough line can say what the game should
  answer (`take key => Taken.`), so a change of wording is caught too.

## The player

- **Notes on screen:** the player's NOTEs in a panel beside the story,
  read-only.
- **Tab completes a word:** while typing, Tab finishes a word the game
  knows (a name here, a verb). Still typed; nothing is clicked.
- **The story as a book:** download the transcript as a tidy web page or
  ebook, with bookmarks as chapters.
- **Sound:** a quiet sound for a room or a timer (`sound_file:`), off
  unless the player turns it on.

## The website

- **Embedding:** a snippet to put a game in a frame on any web page.
- **Collections:** a list of games around a theme, to share as one link.
- **Author pages:** an author's games together, from the gallery.

## No, for now

Ideas we've decided against for the time being. They can come back.

- **Topics:** TOPICS (or ASK KEEPER ABOUT, on its own) lists what someone
  can still be asked about, from their conversation's options.
- **Puzzle chain: authors describe a scripted step:** an optional `step:`
  on a handler, e.g. `step: { text: Give the bread to the child, needs:
  [bread] }`, that the chain uses as written. Precise, but something more
  to keep in step with the script.
- **Puzzle chain: learn from the walkthrough:** note what each command
  made possible when the editor replays it ("command 7, GIVE BREAD TO
  CHILD: the coin appeared, the gate opened"), and use that wherever
  reading the game falls short. True to how the game plays, but only for
  what the walkthrough does.
- **Achievements:** optional `achievements:` a game can award, shown with
  the endings found.
- **Ratings and play counts** for gallery games, kept privately in each
  browser first.
- **A "Game jam" page:** a theme, a deadline, and the entries.
- **Translations:** the player's and editor's own words in other
  languages, starting from the stdlib's language files.
