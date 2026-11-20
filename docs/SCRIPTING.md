# Scripting tutorial: Campfire

The [Midnight Snack tutorial](../README.md#tutorial-midnight-snack) builds a
game with almost no code. This one goes further: you write Lua to make
objects react, keep count of things, run code on a timer and end the game
in different ways.

You'll build **Campfire**. The sun is going down, and the player has to
gather wood, get a fire going and keep it burning long enough to toast
marshmallows. Along the way you'll use:

- event handlers (`before_`, `on` and `after_`) and `block()`
- `player.capacity`, changed by a backpack
- custom verbs
- a Lua script file of helper functions
- `vars` for story-wide values
- object state and `${...}` templates that call your functions
- timers started, restarted and read from scripts
- `random()` and `here`
- several endings
- a character to talk to, with a conversation menu

The finished game is in
[`spec/examples/campfire/`](../spec/examples/campfire/game.yaml). It
assumes you've done Midnight Snack, so rooms, objects, traits and endings
aren't explained again.

## Lua in two minutes

Handlers are written in [Lua 5.4](https://www.lua.org/manual/5.4/). This
is all the Lua the tutorial needs:

```lua
-- A comment.
local n = 3                       -- a local variable
if n > 2 and not done then        -- `and`, `or`, `not`
  say("big")
elseif n == 2 then                -- `==` equal, `~=` not equal
  say("two")
else
  say("small: " .. n)             -- `..` joins text
end
local calls = { "hoot", "rustle" }   -- a list; indexes start at 1
say(calls[1], " and ", #calls)       -- `#` is the length: "hoot and 2"
```

A one-line handler can be a single expression, like `end_game("fed")`.
Longer ones go in a YAML block with `|`, as below.

## Step 1: the campsite

Create `games/campfire/game.yaml`:

```yaml
format: eifgb/0.1

game:
  id: campfire
  title: "Campfire"
  version: "1.0"
  start: campsite
  intro: |
    The sun is sinking behind the pines. If you want a fire before dark,
    you'd better get moving.

player:
  capacity: 3

rooms:
  campsite:
    name: Campsite
    description: |
      A flat clearing beside your tent. In the middle is a ring of
      blackened stones: the fire pit. A trail leads north into the woods.
    exits:
      north: woods

  woods:
    name: Pine Woods
    description: |
      Tall pines, and a carpet of needles. Fallen branches lie everywhere.
      The campsite is back to the south.
    exits:
      south: campsite

objects:
  backpack:
    name: backpack
    synonyms: [pack, bag]
    in: campsite
    traits: [portable, wearable, container, open]
    description: A battered hiking pack.

  matches:
    name: box of matches
    synonyms: [matches, box]
    in: backpack
    traits: [portable]
    description: A small box of strike-anywhere matches.

  marshmallows:
    name: bag of marshmallows
    synonyms: [marshmallows, marshmallow]
    in: backpack
    traits: [portable, edible]
    description: Fat, white and squishy.

  tent:
    name: tent
    in: campsite
    traits: [scenery]
    description: Small, green and slightly lopsided.
```

`player: { capacity: 3 }` means the player can carry a total weight of 3.
Every thing is size 1 unless it says otherwise, and a thing's weight is
its size plus everything inside it. So the backpack, holding the matches
and marshmallows, weighs 3: the player can pick it up, but then their
hands are full.

```
Campsite
A flat clearing beside your tent. In the middle is a ring of blackened stones:
the fire pit. A trail leads north into the woods.
You can see a backpack here.
In the backpack you can see a box of matches and a bag of marshmallows.
Known exits: north.
```

## Step 2: a backpack that frees your hands

Carrying the pack in your arms takes up all of your capacity. Wearing it
should free your hands. Add an `on:` section to the backpack:

```yaml
  backpack:
    name: backpack
    synonyms: [pack, bag]
    in: campsite
    traits: [portable, wearable, container, open]
    description: A battered hiking pack.
    on:
      after_wear: |
        player.capacity = 6
        say("With the pack on your back, your hands are free.")
      after_take_off: player.capacity = 3
```

Every verb has three moments you can hook into, named after the verb's ID
(`wear`, `take_off`, `take`, `open`, …):

| Handler | Runs | Typical use |
|---|---|---|
| `before_wear` | before the built-in action | refuse it with `return block("...")` |
| `wear` | *instead of* the built-in action | replace it completely; `return block()` to count it as failed |
| `after_wear` | after the action succeeded | react to it |

If WEAR fails, for example because the player isn't holding the pack
("You need to be holding the backpack first."), `after_wear` doesn't run,
so the capacity can't go up by mistake. In a handler, `self` is the object it
belongs to, `player` is the player, and `say(...)` prints text.

```
> wear it
You put on the backpack.
With the pack on your back, your hands are free.
```

Lowering a capacity never drops anything: taking the pack off just stops
the player picking up more until they've put something down.

## Step 3: gathering wood

Players should be able to `gather branches` in the woods. That needs
three things: a verb, something to gather, and code that counts the wood.

**A custom verb.** Add a `verbs` section:

```yaml
verbs:
  gather:
    patterns: ["gather {target}", "collect {target}"]
    default: "There's nothing to gather."
```

`{target}` is the object the player names. `default` is the reply when
nothing handles the verb, so `gather tent` says "There's nothing to
gather." without any code from you.

**The branches, and the wood they become.** Add two objects:

```yaml
  branches:
    name: fallen branches
    synonyms: [branches, branch, sticks, wood]
    in: woods
    traits: [scenery]
    description: Dry, and plenty of them.
    on:
      gather: gather_wood()

  firewood:
    name: armful of firewood
    synonyms: [firewood, armful, pile]
    traits: [portable]
    size: 2
    description: |
      Dry pine branches. ${if vars.wood > 1}Enough for a good fire.${else}Not much yet.${end}
```

The firewood has no `in:`, so it starts **off-stage**: it exists, but
it's nowhere in the world until a script moves it somewhere. Its `size: 2`
plus the pack's weight of 3 is too much to carry, unless the pack is worn.

**A story-wide count.** How much wood the player has is stored in `vars`,
declared at the top level:

```yaml
vars:
  wood: 0
```

Scripts read and change it as `vars.wood`. Values in `vars` and in object
state are saved, restored and undone automatically. That's the reason
for the next rule.

**A script file.** `gather_wood()` is a function. Longer code is easier
to read in its own file, so add this to the top level of `game.yaml`:

```yaml
scripts:
  - scripts/camp.lua
```

and create `games/campfire/scripts/camp.lua`:

```lua
-- GATHER BRANCHES: adds an armful to the firewood the player carries.
function gather_wood()
  local wood = world.firewood
  if wood.location ~= nil and not wood:is_in(player) then
    say("You've already started a pile of firewood. Go and get it first.")
    return
  end
  -- :move_to ignores capacity, so check it ourselves.
  if not player:can_hold(wood) then
    say("Your hands are too full to carry any branches.")
    return
  end
  wood:move_to(player)
  vars.wood = vars.wood + 1
  if vars.wood == 1 then
    say("You gather an armful of dry branches.")
  else
    say("You add more branches to the pile in your arms.")
  end
end
```

- **`world.firewood`** is the object with the ID `firewood`. Fields like
  `.location`, `.name` and its state are read with a dot. Methods use a
  colon: `:is_in(holder)`, `:move_to(place)`, `:contents()`, `:has(trait)`
  and `:can_hold(thing)`.
- **`wood.location ~= nil`** is true once the firewood is somewhere in the
  world. If the player dropped it in the campsite, gathering more doesn't
  teleport it back.
- **`:move_to` doesn't check sizes**, because scripts sometimes need to
  break the rules. `player:can_hold(wood)` asks whether it would fit the
  way TAKE would.
- **Script files may only define functions.** A top-level `count = 0`
  would be lost on save and undo, so it's refused. Keep values like that
  in `vars` or in object state.

Try it with the pack in your hand, then worn:

```
> gather branches
Your hands are too full to carry any branches.
```

```
> gather branches
You gather an armful of dry branches.

> gather branches
You add more branches to the pile in your arms.
```

## Step 4: lighting the fire

The fire pit gets a `burning` state, and LIGHT FIRE checks everything
needed before it lights. LIGHT is a standard verb (it lights lamps and
anything else with the `light_source` trait), so there's no verb to add:
the pit's own handlers hook into it. Add the pit:

```yaml
  fire_pit:
    name: fire pit
    synonyms: [pit, fire, campfire, stones, ring]
    in: campsite
    traits: [scenery]
    state: { burning: false }
    description: |
      A ring of blackened stones. ${if self.burning}${fire_words()}${else}It's cold and empty.${end}
    on:
      before_light: |
        if self.burning then
          return block("It's already burning.")
        end
        if not world.matches:is_in(player) then
          return block("You need something to light it with.")
        end
        if vars.wood < 2 then
          return block("You'll need more wood than that. At least two armfuls.")
        end
      light: |
        burn_wood(2)
        self.burning = true
        start_timer("fire_burns")
        say("The match flares, the kindling catches, and soon you have a crackling fire.")
        score(5)
```

- **`before_light` checks, `light` acts.** Each check that fails returns
  `block(...)`, which prints the message and stops there. If none fails,
  the handler returns nothing and `light` runs. Keeping checks separate
  from the action means another handler can add its own checks later.
- **`:is_in` looks inside containers.** The matches are in the worn
  backpack, which is on the player, so `world.matches:is_in(player)` is
  true.
- **`self.burning = true`** changes the pit's state. The description's
  `${if self.burning}` picks it up, and because the description talks
  about `burning` itself, the automatic "It's burning." sentence is left
  out.
- **`score(5)`** adds points. Set `max_score: 10` under `game:` so SCORE
  says "out of a possible 10".
- **LIGHT FIRE WITH MATCHES** works too: the standard verb has a `{tool}`
  pattern. The handler doesn't need it, but a different game could check
  `args.tool`.
- **Hooking a standard verb:** `light` on an object runs instead of the
  built-in LIGHT, for that object only. Anything else the player tries to
  light gets the standard reply.

`fire_words()` and `start_timer` come in the next step. Add `burn_wood` to
`camp.lua`; it uses up wood, and puts the firewood back off-stage when
there's none left:

```lua
function burn_wood(n)
  vars.wood = vars.wood - n
  if vars.wood <= 0 then
    vars.wood = 0
    world.firewood:move_to(nil)
  end
end
```

## Step 5: a fire that burns down

A fire shouldn't burn forever. Give it a timer that puts it out eight
actions after it's lit:

```yaml
timers:
  fire_burns:
    after: 8
    running: false
    do: world.fire_pit.burning = false
    say: Your fire sputters and dies down to embers.
```

`running: false` means it doesn't start with the game. The `light` handler
starts it with `start_timer("fire_burns")`.

**Showing how long is left.** Add `fire_words` to `camp.lua`.
`timer_left` gives the number of actions until a timer is due:

```lua
-- How a burning fire looks, from how many actions it has left.
function fire_words()
  local left = timer_left("fire_burns")
  if left > 5 then
    return "A fire is crackling merrily in it."
  elseif left > 2 then
    return "The fire is burning lower."
  else
    return "The fire is almost out. It needs more wood."
  end
end
```

The pit's description calls it with `${fire_words()}`. Anything in
`${...}` is a Lua expression, so templates can call your functions.

```
> x fire
A ring of blackened stones. A fire is crackling merrily in it.

> z
Time passes.

> z
Time passes.

> z
Time passes.

> x fire
A ring of blackened stones. The fire is burning lower.

> z
Time passes.

> z
Time passes.

> x fire
A ring of blackened stones. The fire is almost out. It needs more wood.
Your fire sputters and dies down to embers.
```

**Adding wood restarts the timer.** Starting a timer that's already
running leaves it alone, so stop it first. Add a verb:

```yaml
  stoke:
    patterns: ["stoke {target}", "feed {target}", "add wood to {target}"]
    default: "That doesn't need feeding."
```

and two more handlers to the fire pit's `on:`:

```yaml
      before_stoke: |
        if not self.burning then
          return block("There's no fire to add wood to.")
        end
        if vars.wood < 1 then
          return block("You don't have any wood.")
        end
      stoke: |
        burn_wood(1)
        stop_timer("fire_burns")
        start_timer("fire_burns")
        say("You add the wood, and the fire roars back up.")
```

Patterns can include fixed words: in `add wood to {target}`, "wood" has
to be typed as it is, and only the fire is looked up.

## Step 6: nightfall, and things in the dark

Two more timers give the game a deadline and some atmosphere:

```yaml
  nightfall:
    after: 25
    do: |
      if world.fire_pit.burning then
        say("Night falls. Your fire holds back the dark.")
      else
        end_game("cold")
      end

  owl:
    every: 4
    do: |
      if here == world.woods then
        local calls = { "An owl hoots somewhere above you.",
                        "Something rustles in the needles.",
                        "A branch creaks in the wind." }
        say(calls[random(1, #calls)])
      end
```

- **`nightfall`** runs once, 25 actions into the game. Its `do` decides
  what happens: the game ends only if the fire is out, and otherwise the
  player gets a message. Calling `end_game` from code works anywhere,
  not just in timers.
- **`owl`** repeats every 4 actions, but only says something when the
  player is in the woods. `here` is the room the player is in.
- **`random(1, 3)`** picks a whole number from 1 to 3. Always use
  `random()`, not Lua's `math.random`, which isn't available: the game's
  random numbers are saved and undone with everything else, so UNDO can't
  be used to re-roll.

Add the ending:

```yaml
endings:
  cold:
    kind: loss
    text: |
      Darkness swallows the clearing. You crawl into the tent, cold and
      hungry, and listen to the owls all night.
```

## Step 7: toasting marshmallows

Finally, the reward. Add a verb:

```yaml
  roast:
    patterns: ["roast {target}", "toast {target}", "cook {target}"]
    default: "You can't roast that."
```

handlers on the marshmallows:

```yaml
    on:
      before_roast: |
        if not world.fire_pit.burning then
          return block("You need a fire first.")
        end
        if here ~= world.campsite then
          return block("The fire is back at the campsite.")
        end
      roast: |
        score(5)
        end_game("cozy")
```

and the winning ending:

```yaml
  cozy:
    kind: win
    text: |
      You toast a marshmallow until it's golden, then another, then a
      third. The stars come out one by one.
```

The marshmallows don't need to be taken out of the pack: things in an
open container the player carries are within reach.

Save a winning walkthrough as `games/campfire/walkthrough.txt`:

```
take pack
wear it
n
gather branches
gather branches
gather branches
s
light fire
x fire
add wood to fire
roast marshmallows
```

```
./target/debug/eifgb play games/campfire/game.yaml --commands games/campfire/walkthrough.txt --seed 1
```

```
> add wood to fire
You add the wood, and the fire roars back up.

> roast marshmallows
[Your score has gone up by 5.]

You toast a marshmallow until it's golden, then another, then a third. The
stars come out one by one.

*** You have won ***
```

`--seed` fixes the random numbers, so the owl says the same things on
every run.

## Step 8: a ranger to talk to

Players like someone to ask for help. Add a ranger station east of the
campsite, with a ranger who stays put so she's always there to talk to.

Give the campsite an exit east, and mention it:

```yaml
  campsite:
    name: Campsite
    description: |
      A flat clearing beside your tent. In the middle is a ring of
      blackened stones: the fire pit. A trail leads north into the woods,
      and a lit window glows to the east.
    exits:
      north: woods
      east: station

  station:
    name: Ranger Station
    description: |
      A snug wooden hut, warm from a stove in the corner. Maps of the park
      cover the walls. The campsite is back to the west.
    exits:
      west: campsite
```

Characters go in their own section:

```yaml
characters:
  ranger:
    name: park ranger
    synonyms: [ranger, woman]
    in: station
    behavior: conversationalist
    talk: ranger_talk
    description: A ranger in a wide-brimmed hat, leaning on the counter.
```

- **`behavior: conversationalist`** means she never moves. Other
  behaviours make characters wander, trade or hunt the player; see
  [Characters](../README.md#characters-npcs-and-enemies). A character with
  `talk` is a conversationalist even if you leave this out, but saying so
  makes the file easier to read.
- **`talk: ranger_talk`** names the conversation that TALK TO RANGER opens
  (and ASK RANGER, or ASK RANGER ABOUT THE TRAIL).

A conversation is a **choice graph**: nodes with text, and numbered
options that lead to other nodes. Add a `choices` section:

```yaml
choices:
  ranger_talk:
    start: hello
    nodes:
      hello:
        text: '"Evening," says the ranger. "Planning a fire?"'
        goto: menu
      menu:
        text: '"What can I help you with?"'
        options:
          - text: "Ask how to build a good fire"
            goto: fire
          - text: "Ask about the noises in the woods"
            say: '"Owls, mostly. They won''t bother you."'
            goto: menu
          - text: "Tell her the fire is going"
            when: world.fire_pit.burning
            say: '"Nicely done. Keep feeding it and it''ll last the night."'
            goto: menu
          - text: "Say goodnight"
            say: '"Goodnight. Mind the embers."'
            end: true
      fire:
        text: |
          "Two armfuls of dry branches to get it going, from the woods up
          north. Then add more before it dies down."
        goto: menu
```

- **`start`** is the first node. A node shows its `text`, then either
  offers its `options` or moves straight on with `goto`. Here `hello` is
  said once, and `menu` is where the conversation returns to.
- **`say`** on an option is the reply to picking it; **`goto`** is where
  to go next; **`end: true`** closes the conversation.
- **`when`** is a Lua condition. "Tell her the fire is going" only
  appears once the fire is lit.
- Options and nodes can also have **`do:`**, Lua that runs when they're
  picked or reached, to change `vars`, give the player something, or
  score points.

```
> talk to ranger
"Evening," says the ranger. "Planning a fire?"
"What can I help you with?"
  1. Ask how to build a good fire
  2. Ask about the noises in the woods
  3. Say goodnight

> 1
"Two armfuls of dry branches to get it going, from the woods up north. Then
add more before it dies down."
"What can I help you with?"
  1. Ask how to build a good fire
  2. Ask about the noises in the woods
  3. Say goodnight

> 3
"Goodnight. Mind the embers."
```

While a conversation is open, the player picks by number. Talking to
someone takes a turn, but picking options doesn't, so a chat with the
ranger costs only one action before nightfall.

## When a script goes wrong

**`check` catches what it can before you play.** Timer, ending and choice
names written as text in a call are checked, with suggestions:

```
error: objects.fire_pit.on.light: start_timer("fire_burn"): no such timer (did you mean 'fire_burns'?)
```

`check` also loads your script files, so Lua syntax errors and top-level
code that isn't a function are reported:

```
error: script error in scripts:scripts/camp.lua: scripts/camp.lua:46: script files may only define functions at the top level; keep story state in vars ('counter')
```

**Other mistakes show up when the code runs.** The game reports them in
brackets, says where they came from, and carries on:

```
> gather branches
[script error in objects.branches.on.gather: vars.wod isn't declared; add it
under 'vars:' in the game file]
```

That's why a walkthrough is worth keeping: replaying it runs your code
and shows any errors straight away.

**The rules scripts run under:**

- Story state lives in `vars` and in object state, never in Lua variables
  that outlive a handler. Everything else is lost on save and undo, so
  the sandbox refuses new globals.
- Objects can't be stored in `vars` or state. Store the ID instead
  (`vars.holding = world.firewood.key`) and look it up with `world[id]`.
- `key`, `name`, `kind` and `location` can't be assigned. Use
  `:move_to(place)` to move things, or `:move_to(nil)` to take them
  off-stage.
- A script that runs too long, such as an endless loop, is stopped with
  an error.

## Where to go next

- [The advanced tutorial](ADVANCED.md) builds a fight between rival
  factions of monsters, all of whom want the player dead.
- [`spec/examples/lighthouse/`](../spec/examples/lighthouse/game.yaml)
  uses a menu puzzle (`open_choice`), a `parse_failed` handler for
  commands the game didn't understand, and more timers.
- The [README cheat sheet](../README.md#cheat-sheet) lists every Lua
  function and field.
- [`spec/FORMAT.md`](../spec/FORMAT.md) is the full reference, including
  the exact order handlers run in.
