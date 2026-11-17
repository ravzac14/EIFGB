# Ideas for later

Things a creative player might try, that we'd like the engine to support
one day. Each builds on something that already exists. Built so far from
the same list: disguises, bait, corpse stories, hiding, making noise and apologies (calming
down, by asking).

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
