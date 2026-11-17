# Ideas for later

Things a creative player might try, that we'd like the engine to support
one day. Each builds on something that already exists. Built so far from
the same list: disguises, bait, corpse stories, hiding, making noise, apologies (calming
down, by asking), haggling, and taming, gifts and bribes.

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
