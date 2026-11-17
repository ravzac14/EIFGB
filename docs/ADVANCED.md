# Advanced tutorial: Blackfang Pass

This tutorial builds a fight. A wolf pack and a goblin war band hate each
other, an undead guard hates everyone, and all of them want the player
dead. The player can't win by charging in. The way through is to set the
monsters on each other.

You'll use:

- `factions`, with `hostile_to` and `eats_corpses`
- `hunter` and `guard` behaviours, with `range` and `wander`
- `health`, `damage` and weapons
- corpses, and the words players use for them
- an `act` script that changes a character's mind when it's hurt
- a `defeated` handler, and `corpse: false`

The finished game is in
[`spec/examples/blackfang/`](../spec/examples/blackfang/game.yaml). It
assumes you've done the [scripting tutorial](SCRIPTING.md), and the
[Characters](../README.md#characters-npcs-and-enemies) section of the
README covers each behaviour.

## Step 1: the pass

Six rooms. The player starts at the trailhead. North is a crossroads,
with the wolves' den to the east, the goblins' camp to the west, and the
shrine gate to the north.

```yaml
format: eifgb/0.1

game:
  id: blackfang
  title: "Blackfang Pass"
  version: "1.0"
  start: trailhead
  intro: |
    The shrine at the top of Blackfang Pass holds the last sunstone. Between
    you and it: a wolf pack, a goblin war band, and whatever guards the
    shrine gate. None of them like visitors. Luckily, none of them like
    each other either.

player:
  health: 20
  damage: 1

rooms:
  trailhead:
    name: Trailhead
    description: |
      The path into the pass starts here, between two leaning boulders.
      It climbs north to a crossroads.
    exits:
      north: crossroads

  crossroads:
    name: Crossroads
    description: |
      Three trails meet in a bowl of broken rock. Howls drift from a den
      to the east; smoke rises from a camp to the west. North, the path
      climbs to the shrine gate. The trailhead is back to the south.
    exits:
      south: trailhead
      east: den
      west: camp
      north: gate_yard

  den:
    name: Wolf Den
    description: A hollow under the rocks, littered with gnawed bones.
    exits:
      west: crossroads

  camp:
    name: Goblin Camp
    description: Ragged tents around a smoky fire.
    exits:
      east: crossroads

  gate_yard:
    name: Shrine Gate
    description: |
      A courtyard of cracked flagstones before an iron gate. Beyond it,
      steps climb to the shrine.
    exits:
      south: crossroads
      north:
        to: shrine
        through: iron_gate

  shrine:
    name: Shrine
    description: A small stone shrine, open to the sky.
    exits:
      south: gate_yard
```

`player: { health: 20 }` lets the player be hurt and die. With health,
the game adds a "You have died." ending for you. `damage: 1` is what
their bare fists do.

The objects are a weapon, the key to the gate, the gate itself, and the
prize:

```yaml
objects:
  spear:
    name: hunting spear
    synonyms: [spear]
    in: trailhead
    traits: [portable]
    damage: 3
    description: Long, light and sharp.

  iron_key:
    name: iron key
    synonyms: [key]
    in: chief
    traits: [portable]
    description: A heavy iron key on a leather thong.

  iron_gate:
    name: iron gate
    synonyms: [gate]
    in: gate_yard
    traits: [door, openable, lockable, scenery]
    state: { locked: true }
    unlocks_with: iron_key
    description: Black iron bars, green with age.

  sunstone:
    name: sunstone
    synonyms: [stone]
    in: shrine
    traits: [portable]
    description: It glows like a coal, warm in the hand.
    on:
      after_take: end_game("won")

endings:
  won:
    kind: win
    text: |
      The sunstone blazes in your hand. Behind you, the pass falls quiet.
```

- **`damage: 3`** makes the spear a weapon: `kill wolf with spear` does
  3 damage instead of 1.
- **`in: chief`**: the key starts in the goblin chief's hands. Things a
  character carries are dropped where they fall.

## Step 2: factions

A faction is a side. Each one says who it attacks:

```yaml
factions:
  wolves:
    hostile_to: [goblins, player]
    eats_corpses: true
  goblins:
    hostile_to: [wolves, player]
  undead:
    hostile_to: [wolves, goblins, player]
```

- **`hostile_to`** lists other factions, and `player` for the player.
  Hostility only goes one way: goblins attacking wolves doesn't make
  wolves attack goblins. Here each side happens to hate the other.
- **`eats_corpses`** makes members eat the bodies of the fallen.
- A character without a faction keeps the simple rule: hunters and
  guards attack the player, and nobody else.

## Step 3: the wolves and the goblins

```yaml
characters:
  alpha:
    name: alpha wolf
    synonyms: [alpha, wolf]
    in: den
    faction: wolves
    behavior: hunter
    range: 1
    wander: 0
    health: 10
    damage: 3
    description: A huge grey wolf with a torn ear.

  wolf:
    name: young wolf
    synonyms: [wolf]
    in: den
    faction: wolves
    behavior: hunter
    range: 1
    wander: 0
    health: 6
    damage: 2
    description: Lean and hungry.

  chief:
    name: goblin chief
    synonyms: [chief, goblin]
    in: camp
    faction: goblins
    behavior: hunter
    range: 1
    wander: 0
    health: 10
    damage: 2
    description: A squat goblin in a helmet two sizes too big.

  grunt:
    name: goblin grunt
    synonyms: [grunt, goblin]
    in: camp
    faction: goblins
    behavior: hunter
    range: 1
    wander: 0
    health: 6
    damage: 2
    description: Mostly teeth.
```

- **`behavior: hunter`** goes after enemies: anyone its faction is
  hostile to.
- **`range: 1`** means it only notices enemies one room away or closer.
  Without it, the wolves would smell the player from anywhere.
- **`wander: 0`** keeps them home until something comes within range.
  Hunters otherwise prowl at random a quarter of the time.

**How a hunter picks its target.** Each turn it looks for the nearest
enemy it can reach. If one is in the same room, it attacks. Otherwise it
takes one step toward them. When the player and a monster are equally
close, **the player comes first**. They all want the player most.

So walking into the crossroads brings both packs at once:

```
> n

Crossroads
Three trails meet in a bowl of broken rock. Howls drift from a den to the
east; smoke rises from a camp to the west. North, the path climbs to the
shrine gate. The trailhead is back to the south.
Exits: south, east, west and north.
The alpha wolf arrives.
The young wolf arrives.
The goblin chief arrives.
The goblin grunt arrives.
```

Stand your ground and everyone attacks you:

```
> z
Time passes.
The alpha wolf attacks you.
The young wolf attacks you.
The goblin chief attacks you.
The goblin grunt attacks you.
```

But step back south, and the nearest enemy each of them can see is no
longer you. It's the other pack, right there in the crossroads. You
can't see the fight, but you can hear it:

```
> s

Trailhead
The path into the pass starts here, between two leaning boulders. It climbs
north to a crossroads.
Exits: north.
You hear snarls and the clash of a fight somewhere north.

> z
Time passes.
You hear snarls and the clash of a fight somewhere north.
```

Characters take their turns in the order they're declared, after the
player's action. Each attacks the first enemy it finds, so both wolves
gang up on the goblin chief while the goblins go for the alpha.

## Step 4: corpses and scavengers

A defeated character drops what it carries and leaves a body: "a goblin
chief (dead)". Players can call any body a `corpse`, `body`, `remains`,
`carcass` or `bones`. Hitting one says it's already dead, and talking to
one gets nowhere.

The wolves have `eats_corpses`. When there's no enemy in the room and a
body is, a wolf spends its turn eating. That's also audible next door:

```
> z
Time passes.
You hear tearing and crunching somewhere north.
```

Back at the crossroads, the goblins are gone. Only the key the chief
dropped is left, and a wolf that's still hungry:

```
> n

Crossroads
...
You can see an iron key and a young wolf here.
Exits: south, east, west and north.
The young wolf attacks you.
```

Goblins don't eat corpses, so if they had won, the wolves' bodies would
still be lying there.

## Step 5: a cowardly alpha

Built-in behaviours are a good start, but a character's own `act` script
replaces its behaviour completely. The alpha should be brave until it's
badly hurt, then slink back to its den, and fight there only if it's
cornered. Add to the alpha:

```yaml
    act: |
      if self.health > 3 then
        self:behave()
      elseif not self:is_in(world.den) then
        self:step_toward(world.den)
      elseif self:is_with(player) then
        self:behave()
      end
```

- **`self:behave()`** does whatever the `hunter` behaviour would, so the
  script only needs to handle what's different.
- **`self.health`** is its current health. `player.health` works the
  same way, and scripts can change either (`player.health = 20` heals).
- **`self:step_toward(world.den)`** heads one room closer to a place or
  a character. `self:is_with(player)` is true when they share a room.
- Other methods for scripts: `self:wander()`, `self:go("west")`,
  `self:attack(target, weapon)`, `self:eat(corpse)`,
  `self:is_hostile_to(other)` and `self:distance_to(target)`.

The alpha does most of the damage in the fight and ends it badly
wounded, so it limps home instead of chasing you. If you go looking for
it, it fights:

```
> e

Wolf Den
A hollow under the rocks, littered with gnawed bones.
You can see an alpha wolf here.
Exits: west.
The alpha wolf attacks you.

> x alpha
A huge grey wolf with a torn ear. The alpha wolf is badly wounded.
The alpha wolf attacks you.
```

## Step 6: the guard at the gate

The skeleton never leaves the gate. It's a `guard`: it attacks any enemy
in its own room, and otherwise stays put. Being undead, it hates
everyone.

```yaml
  skeleton:
    name: skeleton
    synonyms: [skeleton, bones]
    in: gate_yard
    faction: undead
    behavior: guard
    health: 6
    damage: 2
    corpse: false
    description: Old bones, held together by spite.
    on:
      defeated: return block("The skeleton collapses into a heap of dust.")
```

- **`corpse: false`**: it leaves no body when it falls.
- **`defeated`** runs when its health reaches 0. Returning `block(...)`
  replaces the usual "The skeleton is defeated." (and the dropping of
  anything it carried) with your own message. A `defeated` handler can
  also heal the character or move it away, to let it escape or come back.

```
> kill skeleton with spear
You hit the skeleton with the hunting spear.
The skeleton attacks you.

> kill skeleton with spear
You hit the skeleton with the hunting spear.
The skeleton collapses into a heap of dust.
```

## Step 7: the walkthrough

```
take spear
n
# Both packs come running. Step back and let them fight each other.
s
z
z
z
z
z
n
kill young wolf with spear
kill young wolf with spear
take key
n
kill skeleton with spear
kill skeleton with spear
unlock gate
n
take sunstone
```

```
./target/debug/eifgb play spec/examples/blackfang/game.yaml --commands spec/examples/blackfang/walkthrough.txt --seed 1
```

With `wander: 0` everywhere, nothing here is random, so the fight goes
the same way every time. Give the hunters some `wander`, and the packs
will roam and meet in different places on every playthrough. Use
`--seed` to replay one.

## Ideas to try

- **Change the odds:** make the goblins stronger and play again. Since
  goblins don't eat corpses, the bodies pile up.
- **A third side that joins in:** give the undead a `hunter` instead of
  a `guard`, with `range: 2`, and the skeleton joins the brawl at the
  crossroads.
- **Bargaining:** a merchant who only trades with players who've killed
  the goblin chief. Check `world.chief.dead` in a `before_buy` handler.
- **Revenge:** a `defeated` handler on the young wolf that sets
  `vars.alpha_angry`, and an alpha `act` that ignores its wounds when
  it's angry.
