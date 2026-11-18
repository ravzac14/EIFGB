# Sharing your game

Once your game plays the way you want, anyone can play it in their browser
from a link, with nothing to install. You put the game's files online
somewhere public, and the web player fetches them from there.

## Getting a play link

### 1. Put your game online

**On GitHub** (recommended). Make a public repository and put your game's
folder in it: `game.yaml` plus any scripts it lists. Then any of these
links will work:

- the repository: `https://github.com/you/cave` (with `game.yaml` at the top)
- a folder in it: `https://github.com/you/games/tree/main/cave`
- the file itself: `https://github.com/you/cave/blob/main/game.yaml`
- a zip in it: `https://github.com/you/cave/blob/main/cave.zip`

The player always fetches the newest version of the branch you link to.
To share one version that never changes, link to a tag (`tree/v1.0/cave`)
instead of `main`.

**In a gist.** Go to <https://gist.github.com>, add `game.yaml` and any
scripts, and make it public. Gists can't have folders, so a script listed
as `scripts/camp.lua` is found as `camp.lua`. Link to the gist:
`https://gist.github.com/you/0123abcd…`.

**Anywhere else.** Any web address that serves your `game.yaml` (or a zip
of the game) works, as long as the site lets other sites read its files
(it sends `Access-Control-Allow-Origin`). GitHub Pages, Netlify and most
static hosts do. Links must start with `https://`. A link to a folder
looks for `game.yaml` inside it.

### 2. Make the link

Paste your game's link into **Share your game** on the
[Gallery page](../web/gallery.html). It fetches the game, checks it for
mistakes the same way `eifgb check` does, and gives you a link to share,
like:

```
https://<the site>/play.html?game=https%3A%2F%2Fgithub.com%2Fyou%2Fcave
```

## What players see

- A line above the story says where the game came from: "This game comes
  from raw.githubusercontent.com, not from this site."
- Their place is saved automatically in their browser, separately for each
  link, along with any saves they make.
- When you change the game, the link plays the new version. If a player's
  autosave doesn't fit the new version (you renamed a room, say), the game
  starts again from the beginning.

## Getting into the gallery

The gallery lists games anyone can browse and play. To add yours, open a
pull request that adds an entry to the end of `web/gallery.yaml`:

```yaml
  - title: The Cave
    author: Ann
    url: https://github.com/ann/cave
    about: Something is breathing in the dark below the village. A short horror story with a few puzzles.
    tags: [short, horror, puzzle]
```

- `title`, `author` and `about` are required. `about` is one or two
  sentences, up to 300 characters.
- `url` is your game's link, any of the kinds above.
- `tags` are optional, from: `short`, `long`, `beginner`, `puzzle`,
  `story`, `combat`, `mystery`, `horror`, `comedy`, `fantasy`, `sci-fi`
  and `tutorial`.

The site build checks every entry and says what's wrong with any that
aren't right. Before you open the pull request, make sure your game has
no mistakes (**Share your game** on the Gallery page checks it) and can
be finished. Games are listed at the maintainers' discretion; anything
hateful or harmful won't be.

## Is it safe to play other people's games?

Yes. A game's Lua runs inside the web player with no access to your files,
the network, or the rest of the page, and with limits on how long it can
run and how much memory it uses. Everything a game shows is plain text.
