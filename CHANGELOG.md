# Changelog

What's new, newest first. EIFGB is still early (version 0.1), and the game
file format may change before 1.0; anything that changes how existing games
behave will be called out here.

<!-- New entries go under "## Unreleased" here, at the top (add the heading
if it's not there). When the site is deployed, GitHub Actions turns it into
the day's date (web/scripts/date-changelog.mjs); add a title after the date
if you like ("## 2026-11-24: editor tools"). -->

## 2026-11-26

- Sounds: `sound_file:` on a room plays over and over while the player's
  there; on a timer, once when it goes off; and scripts can
  `play_sound("bell.mp3")`. Off unless the player turns on *Sound* (under
  **Aa**), and the game says once that it has some.
- **Notes** in the web player: the journal (goals, promises and your
  NOTEs) in a panel beside the story, kept up to date as you play.
- Embedding: the gallery's games, and **Share your game**, give a snippet
  of HTML that puts a game in a frame on any web page.
- Gossip: what you do to someone (rob them, attack them, a gift, a
  promise broken or kept) gets round, a friend at a time, and people say
  so when they see you: "The miller gives you a hard look: word's got
  round that you robbed the baker." Their own lines go in `heard:`.
- Odd jobs: `jobs:` on a character are errands they pay for. ASK THE
  BAKER FOR WORK, and JOURNAL lists them.
- **Two players** in the web player: in a game with more than one
  playable character, two people take turns on one screen, each as a
  character, passing the turn after every move.
- Busking: SING or PLAY THE FIDDLE in a crowd for a few coins;
  `busking:` on a room says what it pays, and `instrument: true` earns
  more. SING still hums a tune anywhere else.

## 2026-11-24

- What the player learns: `facts:`, taught in conversations (`teaches:`)
  and by reading. WHAT DO I KNOW lists them, someone asked again says "As
  I told you before…", and `knows("mill")` opens new options.
- Places heard of: on the player's map with a "?", and ASK THE GOSSIP
  THE WAY TO THE MILL for directions, after which GO TO goes there.
- Promises: a character `asks:` a favour; PROMISE, and JOURNAL lists it;
  keep it and they're grateful (and give something), break it and
  they'll say so.
- Water: rooms that flood a level at a time, holding your breath over
  your head, something that floats, SWIM, and a sluice to drain it.
- Weight: pressure plates that open a gate while weighed down, and rafts
  that would sink.
- Scents: LET THE DOG SNIFF THE GLOVE, and it leads you to its owner, or
  digs up what's buried.
- Echoes that answer, and whispering galleries.
- The moon: a phase a day, and ways out that are only open at night, by
  day, at certain hours, or under a full moon.
- Copying: COPY THE PASS ONTO THE PAPER, with a pen, makes a forgery that
  passes for the real thing; COPY THE INSCRIPTION writes it into your
  notes.
- In the editor's Test play: **Start with…** starts part-way, in a room,
  carrying what you choose, with doors open, variables set and facts
  known; **Dead ends** finds ways to lose what the walkthrough needs for
  good.
- **How characters sound**, under Problems: each character's lines side
  by side, and a word when two sound alike or one's voice drifts.
- Ropes: TIE THE ROPE TO THE POST and climb down a sheer drop (or back
  up), or TIE UP a sleeping guard.
- Knocking: KNOCK ON THE WALLS for a hollow spot, or KNOCK ON A DOOR and
  whoever's inside may open it.
- Passwords: a doorkeeper who won't let you by until you SAY it.
- Codes can jam after too many wrong tries (`tries:`), say how close a
  wrong one was (`feedback:`), and be dialled in words: DIAL FOUR SEVEN
  ONE.
- Wear: a warning before something breaks, what's left of it
  (`breaks_into:`), and MEND, with something that `mends`.
- Food that goes bad (`spoils:`).
- Hands (`player: { hands: 2 }`): only so much can be held at once, and
  climbing takes a hand free.
- The cold: cold rooms and cold weather, warm coats, soup, and a fire to
  warm up by.
- Notes: WRITE "MEET ME AT THE MILL" ON THE PAPER, and whoever reads it,
  and believes it, goes there and waits.
- Boats and carts that carry you along the ways only they can go, and
  horses to ride: faster, until they tire and need feeding.
- Crowds, always or at certain hours: enemies can't pick you out,
  pickpockets mostly go unnoticed, and nobody leaves tracks.
- Flashbacks: play through a memory somewhere in the past, and what you
  leave there is found in the present.
- An epilogue after the ending, looking back on what the player did.
- Kinds: write a guard or a lamp once (`kinds:`), and have others be
  `like` it, with their own fields on top. In the editor, **Make a kind
  of this**.
- In the editor: **How the descriptions read**, under Problems (words
  used again and again, long or hard descriptions, rooms that all begin
  alike), and **Pacing** in Test play: where the walkthrough brings new
  rooms, things, points and facts, and the long stretches where it
  doesn't.
- In the web player: **Threads**, to read the story so far following one
  character or thing.
- All of these can be set in the editor's details panels too.
- Changed: TIE and KNOCK are built in now. A game's own `verbs: tie:` or
  `knock:` gets a warning that its patterns replace the standard ones,
  and its `default:` reply isn't used; handlers (`on: tie:`) still come
  first, as before.

- The tricks can all be set in the editor's details panels now, without
  writing them in the game file: moods (named in the game's panel, then
  each room's, thing's and character's own words for them), reaching,
  mirrors, coming apart, voices, friends, memories, the characters the
  player can play as, and the earlier ones: combining, marks, fakes,
  watched things, traps, disguises, bait, gifts, taming and bribes.

- More for creative players:
  - Throwing something distracts whoever's watching.
  - Mirrors see round corners and throw light into dark rooms.
  - A pole reaches what's far off or high up.
  - Things come apart (`splits_into:`).
  - CALL OUT AS THE CAPTAIN draws those who answer to him away from
    their posts.
  - PLANT THE RUBY ON THE COOK gets someone else blamed.
- Characters remember what you did (`remembers:`), and word gets round
  their `friends:`.
- Moods: drunk, dreaming or frightened, rooms and things read differently
  (`moods:`), like a room in the dark.
- Several player characters: `playable:` and `player.plays:`, and SWITCH
  TO.
- Names written with a capital ("Tobin") no longer get "the" in front.
- In the editor: Read it aloud, under a description, and "What a player
  might try" for each room.
- In the player: Look back over the story after an ending (and play on
  from any turn), and easier reading: OpenDyslexic, roomier spacing, and
  a sentence at a time.
- The engine the browser downloads is about 30% smaller (3.0 MB to 2.15
  MB), for every game played on the site and every game saved as a page.

- The editor's typo check has a dictionary: a word list kept with the
  site (fetched once; nothing is sent anywhere), so a typo is caught however
  often it's used, and words in the dictionary are never flagged.
- Tab finishes a word in the player, from what the player can see: with
  several matches, Tab goes through them (Shift+Tab back, Esc to undo). It
  works in downloaded games too.

- New things for creative players:
  - Making things: `combines:` turns two things into a third, by TIE,
    COMBINE, PUT IN and the like (TIE THE ROPE TO THE HOOK).
  - Barricades: PUSH THE WARDROBE AGAINST THE DOOR blocks it both ways
    until it's moved.
  - Marks: with chalk (`marks: true`), CHALK AN ARROW leaves a mark in the
    room, shown on LOOK and MAP.
  - Swaps: a character can watch a thing (`watches:`), and a fake
    (`decoy_for:`) can be swapped for it unseen.
  - PLAY DEAD: hunters lose interest, unless they eat the dead.

## 2026-11-23: help for players, editor tools and the puzzle graph

- Making a game with an AI assistant: *Make a game with your AI*, on the
  editor's first page, copies instructions (with the guide to the format)
  to paste into a chat, and opens its reply as a game, chat and all.
  *Copy for your AI*, under Problems, gives a message to send back, and a
  reply pasted over `game.yaml` keeps just the code. The guide for AI
  assistants covers everything new, and a test keeps it up to date.
- The editor is simpler to find your way around: each panel starts with
  the essentials, and folds the rest under headings that say what's set;
  the game's sections are folded too, with how many each has; and a new
  game has a Getting started checklist, to a game that can be won (with
  "Don't show this again", for every game, and a way back under Drawing
  the map).
- Everything new can be set in the editor: text that varies (a box for
  each line, and how it varies), daily routines, scenes, how characters
  feel, and conversation options shown greyed with why.
- The guide starts with the two ways to make a game: in the browser, or
  as files with the command line.
- Fixed: the editor's description helper suggested words like "would" and
  "starts" as things to add.

- Text that varies: a description, message or reply can be a list (each
  in turn), `first:` and `later:`, `cycle:` or `random:`, so WAIT and a
  ticking clock needn't say the same thing every time. `here.visits`
  counts a room's visits.
- Characters keep to daily routines with a clock: `routine: { "6:00":
  bakery, "13:00": square }`.
- Scenes: named stretches of the story that begin and end with
  conditions, shown in RECAP, and readable in scripts (`scenes.storm`).
- Disposition: how each character feels about you, from hostile to
  devoted. Gifts, apologies and kind words warm them; attacks, theft and
  insults cool them, and their side's attitude moves them too. It shows
  when you examine them, in merchants' prices, and in what they'll talk
  about; a hostile character won't talk or trade.
- Conversation options can be shown greyed with why (`why_not:`), can be
  for friends only (`self` is the one you're talking to), and can change
  how they feel (`disposition: 2`).
- Fixed: after UNDO, the game could still remember seeing something the
  undone move had found, so WHERE IS knew where it was.
- GO on its own asks "Which way do you want to go?", WHERE DID I GET on
  its own asks "Where did I get what?", and a command typed in answer to
  such a question (LOOK) is carried out rather than taken as the answer.
- A room or thing with an empty name is now an error: it showed as a gap
  ("You can see a here.").
- `eifgb play … | head` no longer ends with a crash about a broken pipe.

- New in the editor:
  - Conversations as a graph: select one and its parts are drawn on the
    canvas, with a line from each reply to where it leads. Drag to arrange
    them, and to connect a reply to a part or make a new one.
  - Under a description, the words players can already use are
    highlighted, and things it mentions that aren't anything yet are a
    click from being scenery.
  - Possible typos, under Problems: words used once that are a letter off
    another word the game uses, or one of its names.
  - Walkthroughs: a line can say what the game should answer (`take key =>
    Taken.`); `walkthrough-<ending>.txt` checks another ending; and Test
    play lists what the walkthroughs never reach.
  - Wander: a test player that tries 2000 random moves and reports script
    errors (and the moves that led to them), endings, and where nothing
    new happened for a long time.
- Opening a game folder or zip now keeps its walkthroughs.

- WHAT DID SHE SAY? (or WHAT DID THE KEEPER SAY?) shows the last
  conversation again, what you chose too, even in the middle of it.
- MAP describes the places you've been and the ways out of each in words,
  for the terminal and screen readers.
- WHERE IS THE KEY says where you found something you carry (or that
  you've had it from the start), and INVENTORY BY PLACE groups what you
  carry by where you found it.
- Bookmarks, while playing on the website: *Bookmarks → Bookmark this
  moment* marks your last command and what the game said. Read it again
  later, even on another visit, jump to it in the story, or go back to that
  moment in the game.

- New in the editor:
  - Find (Ctrl+Shift+F) searches every room, thing, character, message and
    script for a word, a name or an ID.
  - History lists your recent changes, each saying what it was, to step
    back several at once.
  - Tidy up lays the map out again by its directions, and ways that are
    hard to read on the map (lines that cross, run through a room, or go
    the opposite way to their direction) are marked and listed.
  - A picture already in the game can be used again elsewhere, and the
    editor says how much space pictures take in the browser.
  - Test play can start at any step of the walkthrough, and after an edit
    can play your moves again on the new version.
  - The puzzle chain is a graph: each step says what it needs first, each
    ending lists only the steps it takes, side puzzles are kept apart, and
    a step with more than one way to do it says so. A way that waits on a
    variable says what sets it, or that nothing does.
  - The Puzzles tab says where a player could get stuck: something needed
    to win that can be eaten, burnt, smashed or done away with by a
    script, a key that wears out before the last lock it's for, or a
    choice that rules out the only way to win.
  - Either-or choices: options or handlers that each check a variable
    isn't set and then set it are read as "only one of these". Each says
    what it rules out, endings say which choice they need ("only if the
    player chooses “Take the sword”"), and an ending needing two of them
    can't happen. A way waiting on a value (`vars.side == "rebels"`) needs
    what sets that value.
  - Steps solved in scripts are marked, with a link to the script, and the
    Puzzles tab lists what the chain takes on trust: scripts it can't see
    inside, conditions it can't read, things got from characters. Verb
    handlers are read as the action ("Give the loaf of bread to the hungry
    child", from `args.tool == world.child`), and a merchant's goods are
    bought.
  - The Puzzles tab drafts hints from the puzzle chain: a nudge, a clue and
    the answer for each thing to find and door to unlock, until it's done.
  - The Timers tab shows when each timer can go off, in actions and (with
    a clock) the time of day, and what it does.
- Scripts can ask whether the player has found something yet:
  `world.key.seen` (seen it or held it), for hints that stop once it's found.
- Typos are never fixed behind the player's back any more. SERCH POOL used
  to run SEARCH POOL silently, and a real word the game didn't know could
  be taken for one it did: THINK became THANK. Now the game asks "Did you
  mean SEARCH?", and YES runs it. **This changes how games behave:**
  walkthroughs and recordings with typos in them need the typos fixed.
- New everyday words: THINK (PONDER, THINK ABOUT …), NOD, SHRUG, HUG, CLAP,
  SHIVER, SIGH and YAWN, with stock replies.
- "What do you want to …?" uses the player's own words: SHAKE asks "What do
  you want to shake?", not "…to wake?", and THANK no longer asks "…to
  thanks?".
- "Did you mean…?" is pickier: PONDER no longer suggests ORDER.
- Help for players, in every game:
  - WHERE IS THE KEY (or FIND THE KEEPER) says where you last saw it, and
    GO TO THE KEEPER walks you there. It's a reminder, not a hint: it only
    knows what you've seen.
  - UNDO 3 takes back three moves at once.
  - OOPS LANTERN fixes the word the game didn't know in your last command,
    and a slip too close to more than one word asks "Did you mean TAKE?"
    instead of giving up.
  - BRIEF stops describing places you've already been; VERBOSE goes back.
  - RECAP sums up where things stand, for coming back to a game after a
    while. A restored game mentions it.
  - ASK THE KEEPER ABOUT THE GATE goes straight to the part of the
    conversation about the gate.
  - AGAIN WITH THE IRON KEY repeats your last command with something else.
- BRIEF is now a command, so it's no longer an unknown word.
- Fixed: FEEL FOR THE COIN in the dark no longer counts everything else
  there as found.

## 2026-11-20: notes, how things behave, and the puzzle chain

- Players can keep notes: NOTE THE CODE MIGHT BE 4512 writes one, word for
  word, with no turn taken. JOURNAL and NOTES list them; ERASE NOTE 2 and
  CLEAR NOTES rub them out. They're kept with saves, and UNDO and RESTART
  leave them alone.
- How things behave, with no scripts needed:
  - Flames: `needs_flame` lights (candles) light only from something
    burning; LIGHT THE CANDLE WITH THE MATCHES strikes the match first.
  - Fuel: `fuel` lights burn down, flicker, and go out, or are gone with
    `burns_away` (matches).
  - Liquids: `holds_liquid` and `liquid: water`; FILL, POUR, DRINK, and
    POUR THE WATER ON THE FIRE.
  - Wear: `uses` and `breaks`.
  - Codes: `code: 4512` on a lockable thing; TYPE 4512.
  - Writing: `text`, for READ.
  - Sounds and smells: `sound`, `smell`; LISTEN and SMELL reach next door.
  - Weather: a `weather:` section, `outdoors` rooms, `game.weather` and
    `set_weather`.
  - Timers can go off by `chance: 10%`.
  - Companions are known in the dark, and one that sees in the dark can
    fetch what the player can't see.
- READ is now its own verb (it was a synonym for EXAMINE); LISTEN and SMELL
  are built in. Games' handlers for them still come first.
- Scripts can light and darken rooms: `world.cellar.dark = false`. When
  the light changes from a script or timer (or a lamp running dry), the
  room is described at once, or the darkness is. Scenery that's only a
  light (a light source with no description) isn't something to feel for.
- A timer can be heard only in some places: `heard_in: tower` (a room, a
  list of them, or a thing such as a clock, heard wherever it is).
  Elsewhere it goes off unannounced. In the editor: "It's heard".
- Timers can say something else after the first time: `say_again`. In the
  editor: "After the first time, it says". Left empty, `say` plays every
  time.
- Timers can go off at random: `after: 3-8` or `every: 5-10` picks a
  random number of turns in the range, again each time for `every`. The
  same seed plays the same way, so walkthroughs still replay. In the
  editor, type a range in the timer's turns.
- Feeling your way in the dark: things the player has seen (or put down,
  or felt for) can still be reached in the dark, so TAKE KEY and LIGHT THE
  CANDLE WITH THE LIGHTER work. FEEL AROUND finds what's within reach,
  FEEL FOR THE KEY finds it and picks it up, and LOOK in the dark says
  what the player knows is there. `findable_in_dark: true` (in the
  editor: "Can be found by feel in the dark") for a light switch or
  matches the player can always find.
- The editor's **Puzzles** tab: one way through the game, step by step
  (find, unlock, reach, end), and what can never happen: rooms nobody can
  reach, keys that open nothing, locks with no key, hidden things nothing
  reveals, endings nothing leads to.
- Test play keeps what the game didn't understand, under **Not
  understood**, each with a fix: add the word as another name for
  something in that room, make an unknown verb mean one the game knows,
  or add the word as an object.
- Renaming in the editor reaches everything: changing a room's, object's
  or character's ID updates every reference (exits, doors, `via`, keys,
  reveal and timer rules, characters' things) and the Lua code that names
  it (`world.hall`), in game.yaml and in script files, and the room keeps
  its place on the map. Renaming an ending, conversation, timer, goal or
  verb updates `end_game("…")`, `open_choice("…")`, `start_timer("…")`,
  `start_goal("…")` and a verb's handlers. One UNDO takes it all back.
- Pictures: a room, object or character can have a `picture` (a file
  beside the game, or an https address) and `alt` words for players who
  can't see it. A room's shows above its description and with the map; a
  thing's when it's examined, for clues the player has to see. In the
  editor: a **Picture** section on each, and **Add component → Picture**.
  The Small House starter has a note on the fridge to look at.

## 2026-11-19: talking naturally, goals, the time of day, and hints

- Players can talk more naturally:
  - Lists with commas (TAKE THE KEY, THE COIN AND THE LAMP), MY HAT, THE
    BRASS ONE, THE OTHER ONE, THE SECOND KEY, TWO COINS. Identical things
    (two gold coins) no longer get "which do you mean?".
  - Extra words are fine: CAN I GO EAST, TRY WEST, LET'S GO SOUTH, LOOK
    CLOSELY AT IT, GO EAST PLEASE, SAVE GAME, TAKE IT AGAIN. UNDO UNDO
    undoes twice.
  - Questions: WHAT'S HERE, WHAT AM I CARRYING, CHECK INVENTORY, WHO IS
    HERE, WHAT CAN I DO (suggestions for right now), ABOUT (title, author,
    version).
  - The floor: TAKE THE KEY FROM THE FLOOR, PUT IT ON THE GROUND. X ALL
    looks at everything.
  - LOOK UP / LOOK DOWN where there's no way says "You see nothing special
    that way."
  - A place's name goes there: KITCHEN, GO KITCHEN, next door or anywhere
    visited.
  - HELLO and HELLO KEEPER start a conversation; SAY XYZZY is a verb games
    can hook.
  - Short replies, which games can hook or reword, for JUMP, SIT, LIE
    DOWN, SLEEP, SING, DANCE, PRAY, CRY, LAUGH, YES, NO, THANKS, SORRY,
    XYZZY, and CUT, TIE, DIG, KNOCK, WAVE, POINT.
  - DRINK (new trait `drinkable`), CLIMB, WAKE, and SHOW … TO ….
- Exits can say what they go by: `via: stairs` makes CLIMB THE STAIRS, GO
  DOWN THE STAIRS and CROSS THE BRIDGE work, and the staircase is there on
  both sides. In the editor: **Goes by** on a way out.
- `aliases:`: a game's own words for words it knows (`using: with`, `port:
  west`), anywhere in a command. In the editor: **Words** in the game's
  details.
- Several commands now mean something: DANCE, SING, JUMP and the other
  words above used to be "I don't know the verb". A game's `parse_failed`
  handler no longer sees them; hook the verb instead (`on: { dance: ... }`).
- More of what players type works:
  - AND joins commands: TAKE THE KEY AND OPEN THE DOOR. It still joins
    things, too: TAKE THE KEY AND THE LAMP, and now TAKE ALL BUT THE KEY.
  - USE: USE LAMP lights it, USE HAT wears it, USE KEY ON DOOR unlocks
    it, USE SWORD ON GOBLIN attacks. The game's own handlers for those
    verbs still run. A game that defines its own `use` verb now gets a
    warning, as with any standard verb.
  - GO TO THE KITCHEN walks to a place visited before, a turn a step,
    stopping if anything happens. GO BACK returns to the last room. UNDO
    takes back the whole walk.
  - WAIT 3, WAIT TEN MINUTES, WAIT UNTIL DUSK, WAIT UNTIL 9:30 PM wait,
    stopping as soon as anything happens.
  - X ROOM, SEARCH HERE and LOOK AT THE HALL work on the room; WHERE AM I
    is LOOK; EXITS lists the ways out.
  - Fixed: EXITS no longer means EXIT (go out), SING no longer means S
    (south), and BRIEF is no longer taken as a misspelling of BRIBE.
- Doors can be named by the way they lead: UNLOCK NW DOOR, OPEN THE EAST
  DOOR. When two doors have the same name, "Which do you mean?" says
  which way each one goes, and NORTHWEST is enough as a reply.
- ASK and TELL start a conversation, like TALK TO: ASK KEEPER, ASK
  KEEPER ABOUT THE GATE, TELL KEEPER ABOUT THE STORM. With only one person
  here to talk to, TALK or ASK ABOUT THE GATE needs no name. (ASK … TO is
  still an order.)
- Read aloud, in *Text and colours* while playing: new text is spoken
  with your browser's own voices, at normal speed, slower or faster.
  Escape stops it. Off unless you turn it on.
- A *Very large* text size.
- For screen readers: the map is described in words (where you are, the
  places you've been and their ways out), the story is marked with the
  game's language, and the home page's demo has a proper label.
- Goals: a `goals:` section, each with text, and conditions for when it's
  added and when it's done. JOURNAL lists them, and a notice says when one
  is added or done. In the editor: Add component → Goal.
- The time of day: a `clock:` section starts a clock that moves on with
  each turn, shown on the status line. TIME tells the player; text and
  scripts can check `clock.night`, `clock.hour` and more; `pass_time` and
  `set_clock` move it on; and timers can go off `at: dusk` or at a time.
  In the editor: Add component → Time of day.
- The Small House starter template now has goals, the time of day, and
  a clock that strikes nine.
- Hints: a `hints:` section lists puzzles in the order players meet them,
  each with steps from a nudge to the answer and a `when` that's true while
  it's unsolved. HINT shows the next step for the puzzle the player is on
  (it used to be the same as HELP). The starter templates have hints, and
  the editor can add and edit them (Add component → Hint).
- Record a walkthrough in the editor: play from the start in Test play,
  then **Save as walkthrough**. After every change, the editor replays it,
  says in Test play whether it still wins, and warns under Problems when it
  doesn't. Starter templates and copied examples bring theirs along. Test
  play now always uses the same random numbers, so recordings replay the
  same way.
- Download a game as one web page: the editor's **Download** offers a
  single .html file with the game and engine inside, that plays anywhere,
  even offline. The zip of the game's files is still there too.
- A Parchment look in *Text and colours*: ink on old paper, room names in
  small capitals, and a drop capital at the start of each room's
  description. Games can choose it too, with
  `presentation: { theme: parchment }`.
- *Text appears: letter by letter*, in *Text and colours* while playing:
  new text is typed out like a typewriter. It's off unless you turn it on,
  and any key, click or tap shows the rest at once.
- Starter templates in the editor: start a new game from "A Small House"
  (five rooms, a locked front door and keys to find), "The Locked Door"
  (a dungeon cell, a sleeping guard and an iron door) or "Market Day"
  (someone to talk to, a baker, and someone to help), instead of a blank
  room. Each is a small, finished game to change into your own.
- The editor's "+ Room" button is now "Add component", for anything a game
  can have: a room (added straight away, as before), an object or a
  character (it asks which room first), a door, a side, an ending, a
  conversation, a verb, a timer, a variable or a script file.
- Room colours in the editor, to group rooms into areas (the castle, the
  forest): pick one for each room, and it shows on the player's map too.
  They're kept in `map.yaml`, so the game itself doesn't change.
- An "Editor theme" choice on the editor's map: the site's colours, or
  Blueprint (white lines on blue), so working on the map feels different
  from playing. It's remembered in your browser.
- Room descriptions end with "Known exits:", and list only the ways out
  that are open to the player right now: not an exit whose `when` is
  false, and not a secret door that hasn't been found. Games can turn the
  list off with `settings: { list_exits: false }`, or per room; the
  editor has a switch for both.
- Test play in the editor: play your game in the editor's bottom panel
  while you work on it, from the start or from the room selected on the
  map, with the room you're in lit up on the map. It tells you when you've
  changed the game since starting.
- When a game ends on the website, you can still type: UNDO, RESTORE and
  RESTART work, as the game says.
- For AI assistants: a guide to writing games with one (format, common
  mistakes, a complete checked example), every docs page as Markdown next
  to its HTML, and `llms.txt`, `llms-full.txt` and `sitemap.xml` at the
  site's root.
- Send feedback from anywhere on the site: report something broken or
  confusing, or an idea for playing, the editor or the website. It says
  which page (or which game, room and turn) you were on and which version
  of the site you're seeing, and shows everything it sends first.
- Download a transcript of your game, as a text file, from Saves or when
  the game ends.
- Endings found: a game's card on the home page and in the gallery says
  "You've found 2 of 3 endings", and so does the end of a game. It's kept
  in your browser.
- Games can choose their look on the website: `presentation: { theme:
  terminal, accent: "#c33" }`, or "How it looks" in the editor. Players
  see it unless they've picked their own colours.
- In the editor, drag a selected room's corner to make its box bigger or
  smaller, so a great hall looks bigger than a closet. It's only for
  picturing the map (it's kept in `map.yaml`), and the player's map draws
  rooms the same size.
- Barter: TRADE RING FOR LAMP swaps something you carry for something a
  merchant sells, no money needed, if it's worth enough to them. See
  [Trading](README.md#trading).
- Pushing and climbing: `pushable` things are too heavy to carry but can
  be pushed into the next room (PUSH CRATE NORTH), and `high: true`
  things are out of reach until the player stands on something (CLIMB ON
  THE CRATE). See [Pushing and climbing](README.md#pushing-and-climbing).
  CLIMB UP and CLIMB DOWN now go up and down.
- Light shows through open ways between rooms: in a dark room next to a
  lit one, "Light spills in from the north."; in a lit room next to a dark
  one, "It's dark to the north."
- A map in the player that fills in as you explore: the places you've been,
  the ways between them, doors, and the ways out you haven't tried yet.
  Rooms sit where the author put them in the editor.
- The player scrolls down to show each reply as it arrives; a long reply
  stops at its first line, so you can read it from the beginning.
- Choices (in conversations and menus) are typed by number, like every
  other command, rather than clicked.
- The guide's list of commands every game understands is now a set of
  tables, with the other ways to say each one.

## 2026-11-18: new looks, and a game on the home page

- A tiny game to try at the top of the home page: you're locked in a study.
- New looks in *Text and colours*: Terminal (green or amber, with a soft
  glow and scan lines) and High contrast. A *Very legible* typeface
  (Atkinson Hyperlegible), roomier line spacing, and narrow, medium or wide
  columns. They apply to the player, the editor and the docs.
- The site has an icon, and links to it show a preview card when shared.

## 2026-11-17: the website

Everything now runs in the browser, at
[ravzac14.github.io/EIFGB](https://ravzac14.github.io/EIFGB/).

### Playing

- Play the example games, or your own: drop a game folder, a zip or a
  `game.yaml` on the home page.
- Your place is saved automatically after every command. SAVE and RESTORE
  keep named saves, and saves can be downloaded and loaded on another device.
- A book-like page with light, dark and sepia colours, three text sizes, and
  a layout that works on phones.

### Sharing

- Share a game with a link: `play.html?game=` and the address of a game
  hosted on GitHub, in a gist, or anywhere public.
- A [gallery](https://ravzac14.github.io/EIFGB/gallery.html) of games, which
  you can add yours to with a pull request. See
  [Sharing your game](docs/SHARING.md).
- Other people's games run sealed off from your computer. The sharing guide
  explains what that means, and what a game's host can see.

### Making games: the visual editor

- Draw your map: rooms, the ways between them, doors (locked, secret).
- Put things in rooms, in containers and on surfaces by dragging them.
- Add characters (people to talk to, shopkeepers, enemies, guards,
  companions) and sides that fight each other.
- Write scripts, timers, endings, conversations, your own verbs and
  variables, with problems shown as you type.
- Everything you do is written into your game's file, which you can also
  edit by hand. Download it as a zip at any time.

### Docs

- The guide, both tutorials and the format reference are pages on the site,
  with a page for each example game showing its files.
- The home page's heading is now *Interactive Fiction Game Builder/Player*.
- This changelog, on the website.

### Checking games

- `eifgb check` (and the editor) now find Lua syntax mistakes in every
  script, not just in script files, before the game is played.

## 2026-11-17: characters, and tricks players can find

### Characters

- People and creatures that stay put, talk, wander, trade, hunt, guard,
  follow or defend themselves. See
  [Characters](README.md#characters-npcs-and-enemies).
- Fighting, with health and damage; buying and selling; haggling.
- Sides (factions) that hate each other, eat what's left lying around, and
  turn on the player if provoked in front of them. Patience, apologies,
  pacifists and characters who can't be hurt.

### Tricks players can find

- Disguises, bait, hiding, and bodies that tell how they died.
- Making noise, mimicking calls that only one side answers, and eavesdropping
  through walls.
- Taming, gifts and bribes; throwing things, and companions that fetch;
  naming things (CALL THE DOG REX); giving orders.
- Tracks to read and follow; witnesses; hiding bodies.
- Sleep and sneaking; peeking into the next room; full darkness and light.
- Pickpocketing; fear and morale; traps.
- Fire that spreads, scorches and can be put out; smashing things.

### Tutorials

- The scripting tutorial (Campfire) and the advanced tutorial (Blackfang
  Pass).

## 2026-11-16: the first version

- The game file format: rooms, exits, objects and characters in YAML, with
  Lua for anything special.
- The compiler (`eifgb check`, `eifgb build`), the engine, and
  `eifgb play` to play in a terminal.
- A forgiving parser, undo, saving, menus and dialogue, endings and score.
- Descriptions that mention what's open, locked, empty or worn, without the
  author writing it.
- Doors that work from both sides, hidden things and secret doors,
  containers and surfaces, carrying limits, and timers.
- The README tutorial (Midnight Snack) and the Lighthouse sampler.
