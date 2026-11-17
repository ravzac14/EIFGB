# Ideas for later

Things a creative player might try, that we'd like the engine to support
one day. Each builds on something that already exists. Built so far from
the same list: disguises, bait, corpse stories and hiding.

## Making noise

SHOUT, BANG POT, or ring a bell, and hunters within earshot come running
to the noise. Players could pull packs together wherever they choose,
instead of only where the author planned.

- Builds on: hearing fights and feeding in neighbouring rooms, hunters' `range`.
- Shape: a `noise` action (or `makes_noise: N` on objects/verbs) that sets
  a "last heard" room on hunters within N rooms, reusing the hiding
  feature's "last seen" search.
- Effort: small.

## Haggling, with patience

OFFER 5 COINS FOR LAMP. The merchant counters; each lowball offer costs
patience, and pushed too far they refuse to trade with you at all.

- Builds on: merchants, `price`, `buys_at`, `patience`.
- Shape: OFFER/HAGGLE verbs; a merchant `haggle` percentage (how far
  they'll come down); a refusal state when patience hits 0, separate from
  becoming hostile.
- Effort: medium.

## Taming and gifts

GIVE MEAT TO WOLF while it's wounded or hungry, and it becomes a follower
that fights on your side. GIVE COINS TO GUARD gets you waved through.

- Builds on: bait (`eaten_by`), followers, factions, `revealed_by`-style rules.
- Shape: `tamed_by` / `bribed_by` rules on characters ("give haunch",
  "give (money) 5"); taming switches faction to an allies faction and
  behaviour to `follower`; followers fight the player's enemies.
- Effort: medium.

## Calming down

Patience slowly returns if you stay out of someone's way, and APOLOGISE
TO MONK or PRAY can earn their trust back.

- Builds on: `patience`, `foes`.
- Shape: `calms_after: N` actions without being attacked clears `foes`
  and restores patience; an APOLOGISE verb with a hookable default.
- Effort: small.

## Throwing

THROW SPEAR AT WOLF hits from across the room. THROW STICK sends a
follower dog to fetch it. THROW KEY ACROSS CHASM lands it in the next room.

- Builds on: weapons and `damage`, followers, size and weight.
- Shape: THROW X (AT Y / direction); thrown weapons do damage and land
  in the room; followers with `fetches: true` bring things back;
  `throwable_across` on exits.
- Effort: medium.

## Naming things

CALL THE DOG REX, and from then on "rex" works and the game calls it Rex.

- Builds on: the parser's vocabulary and entity names.
- Shape: per-game renaming: a `name` and extra vocabulary in entity state,
  read by the parser and by `a()`/`the()`; `nameable: true` on characters.
- Effort: medium (names and vocabulary are fixed in the story today).
