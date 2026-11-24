// Threads: the story so far, following one character or thing. The page
// is a run of turns (each a command and what came of it); following a
// thread hides the turns that don't mention it, so the player can read
// everything that happened with the baker, start to finish.

/** The turns on the page: the opening, then each command and what followed. */
export function turnsOf(log: Element): HTMLElement[][] {
  const turns: HTMLElement[][] = [[]];
  for (const node of log.children) {
    const e = node as HTMLElement;
    if (e.classList.contains("diagnostics")) continue;
    if (e.classList.contains("cmd")) turns.push([]);
    turns[turns.length - 1].push(e);
  }
  return turns.filter((t) => t.length);
}

/** Whether text mentions any of the words, as whole words. */
export function mentions(text: string, words: string[]): boolean {
  const t = ` ${text.toLowerCase().replace(/[^a-z0-9']+/g, " ")} `;
  return words.some((w) => {
    const clean = w.toLowerCase().replace(/[^a-z0-9']+/g, " ").trim();
    return clean.length > 1 && (t.includes(` ${clean} `) || t.includes(` ${clean}s `) || t.includes(` ${clean}'s `));
  });
}

/** Shows only the turns that mention the words. Returns how many show, of how many. */
export function follow(log: Element, words: string[]): { shown: number; total: number } {
  const turns = turnsOf(log);
  let shown = 0;
  for (const t of turns) {
    const hit = mentions(t.map((e) => e.textContent ?? "").join(" "), words);
    if (hit) shown++;
    for (const e of t) e.classList.toggle("thread-out", !hit);
  }
  log.classList.add("threading");
  return { shown, total: turns.length };
}

/** Shows every turn again. */
export function unfollow(log: Element) {
  for (const e of log.querySelectorAll(".thread-out")) e.classList.remove("thread-out");
  log.classList.remove("threading");
}
