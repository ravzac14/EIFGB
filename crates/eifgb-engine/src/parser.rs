//! The command parser: the "never leave the player stuck" chain from
//! FORMAT.md. Steps 1 to 6 live here. Steps 7 to 9 (the author hook,
//! the AI fallback and the helpful failure) live in the turn loop,
//! which acts on the [`Failure`] returned from here.
//!
//! 1. Exact match against verb patterns and in-scope vocabulary.
//! 2. Normalization: lowercasing, dropping filler and articles,
//!    rewriting synonym phrases, stemming inflected words, and
//!    resolving pronouns.
//! 3. The synonym lexicon (`Lexicon::phrases`).
//! 4. Typo correction, limited to words this game actually uses.
//! 5. Clarifying questions: [`Parsed::Ambiguous`] and [`Parsed::NeedNoun`].
//!
//! Some extra conveniences: a bare noun means "examine it", commands
//! can be chained with "." or "then", and replies to clarifying
//! questions are merged back into the original command.

use crate::lang::{self, Lang};
use crate::world::World;
use eifgb_ir::{BuiltinAction, Direction, EntityId, PatternToken, Slot, Story, Trait, VerbAction, VerbOrigin, PLAYER};
use std::collections::{HashMap, HashSet};

/// A fully resolved command.
#[derive(Debug, Clone, PartialEq)]
pub struct Command {
    /// Index into `Story::verbs`.
    pub verb: usize,
    /// More than one target only for "all".
    pub targets: Vec<EntityId>,
    pub tool: Option<EntityId>,
    pub text: Option<String>,
    pub direction: Option<Direction>,
    /// The normalized words the command came from. AGAIN replays these.
    pub words: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Parsed {
    Command(Command),
    /// More than one thing matched. The words around the noun are kept so
    /// that the player's answer can be merged back in.
    Ambiguous { before: Vec<String>, after: Vec<String>, options: Vec<EntityId> },
    /// Something is missing: the object ("take"), or the second object
    /// ("fill lamp"), in which case `target` and `word` are set. The
    /// player's next input is appended to `prefix` to complete it.
    /// `way`: what's missing is a direction (GO on its own).
    NeedNoun { prefix: Vec<String>, verb: String, target: Option<EntityId>, word: Option<String>, way: bool },
    Fail(Failure),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Failure {
    Empty,
    UnknownVerb { word: String },
    /// The verb was understood, but not the rest of the command.
    Partial { verb: String },
    UnknownWord { word: String },
    NotHere,
    /// The words only appear in the room's description: "x rocks" when the
    /// room mentions black rocks but there's no rocks object.
    Scenery,
    NoPronoun,
    NoAll,
    AllNone,
}

/// A token that refers directly to an entity. The parser creates these
/// when merging the answer to a "which do you mean?" question.
const ENTITY_TOKEN: char = '#';

pub fn entity_token(id: EntityId) -> String {
    format!("{ENTITY_TOKEN}{}", id.0)
}

#[derive(Debug, Clone, Default)]
struct Caps {
    target: Option<(usize, usize)>,
    tool: Option<(usize, usize)>,
    text: Option<(usize, usize)>,
    direction: Option<Direction>,
}

struct Candidate {
    verb: usize,
    caps: Caps,
    /// The number of literal words in the pattern. When nothing resolves,
    /// the failure from the most specific pattern is the most useful one.
    specificity: usize,
}

enum NounFail {
    Missing,
    NoPronoun,
    NotFound { unknown_word: Option<String> },
    /// The phrase contains a grammar word such as "from" or "with", so it's
    /// a command with missing or misplaced parts, not an unknown thing.
    Structural,
    Ambiguous(Vec<EntityId>),
    NoAll,
    AllNone,
    Scenery,
}

/// Precomputed word tables for one story.
pub struct Grammar {
    lang: &'static Lang,
    phrases: Vec<(Vec<String>, Vec<String>)>,
    filler: Vec<Vec<String>>,
    articles: HashSet<String>,
    pronouns: HashSet<String>,
    all_words: HashSet<String>,
    directions: HashMap<String, Direction>,
    /// The first word of every pattern.
    verb_words: HashSet<String>,
    /// Every word in every entity's vocabulary.
    vocab_words: HashSet<String>,
    /// Every literal word in any pattern ("with", "from", "up"...).
    pattern_words: HashSet<String>,
    /// Each entity's description words (and their stems), minus stopwords.
    /// Indexed like `Story::entities`.
    described: Vec<HashSet<String>>,
    /// Every word in any description, so that unknown-word replies don't
    /// claim not to know a word the game itself printed.
    described_words: HashSet<String>,
    /// "corpse", "body": words for any dead character.
    corpse_words: HashSet<String>,
    /// "and": joins things ("the key and the lamp") or commands.
    and_words: HashSet<String>,
    /// "but", "except": "all but the key".
    except_words: HashSet<String>,
    /// "room", "here": the room the player is in.
    room_words: HashSet<String>,
    my_words: HashSet<String>,
    one_words: HashSet<String>,
    other_words: HashSet<String>,
    ordinals: HashMap<String, u32>,
    numbers: HashMap<String, u32>,
    adverbs: HashSet<String>,
    /// Longest first.
    trailing: Vec<Vec<String>>,
    /// The game's `aliases`, longest first.
    aliases: Vec<(Vec<String>, Vec<String>)>,
}

/// The words of a text's literal parts, including both branches of every
/// `${if}`, lowercased and split on anything that isn't a letter, digit or
/// apostrophe.
fn text_words(text: &eifgb_ir::Text, out: &mut Vec<String>) {
    use eifgb_ir::Segment;
    for seg in &text.0 {
        match seg {
            Segment::Lit(s) => out.extend(
                s.split(|c: char| !(c.is_alphanumeric() || c == '\''))
                    .map(|w| w.trim_matches('\'').to_lowercase())
                    .filter(|w| !w.is_empty()),
            ),
            Segment::Expr(_) => {}
            Segment::If { then, otherwise, .. } => {
                text_words(then, out);
                text_words(otherwise, out);
            }
            Segment::Vary { options, .. } => options.iter().for_each(|o| text_words(o, out)),
        }
    }
}

fn words(s: &str) -> Vec<String> {
    s.split_whitespace().map(str::to_string).collect()
}

/// A name's words, lowercased: "Base of the Lighthouse" → base, of, the, lighthouse.
fn words_of(s: &str) -> Vec<String> {
    s.to_lowercase().split(|c: char| !c.is_alphanumeric() && c != '\'').filter(|w| !w.is_empty()).map(str::to_string).collect()
}

/// Edit distance where swapping two neighbouring letters ("tkae") counts
/// as one edit (optimal string alignment).
fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (n, m) = (a.len(), b.len());
    let mut d = vec![vec![0usize; m + 1]; n + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in d[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            d[i][j] = (d[i - 1][j] + 1).min(d[i][j - 1] + 1).min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                d[i][j] = d[i][j].min(d[i - 2][j - 2] + 1);
            }
        }
    }
    d[n][m]
}

/// Words a typo might have meant, for "Did you mean…?": the nearest few
/// (ties included). Never swapped in unasked: THINK isn't THANK.
fn near_misses(word: &str, candidates: &[String]) -> Vec<String> {
    let n = word.chars().count();
    if n < 3 || word.starts_with(ENTITY_TOKEN) || word.chars().all(|c| c.is_ascii_digit()) {
        return Vec::new();
    }
    // Two letters wrong only in a long word: PONDER isn't ORDER.
    let limit = if n <= 6 { 1 } else { 2 };
    let mut near: Vec<(usize, &String)> = candidates
        .iter()
        .filter(|c| c.chars().count() > 1)
        .map(|c| (distance(word, c), c))
        .filter(|&(d, _)| d > 0 && d <= limit)
        .collect();
    let Some(best) = near.iter().map(|&(d, _)| d).min() else { return Vec::new() };
    near.retain(|&(d, _)| d == best);
    let mut out: Vec<String> = near.into_iter().map(|(_, c)| c.clone()).collect();
    out.sort();
    out.dedup();
    out.truncate(3);
    out
}

impl Grammar {
    pub fn new(story: &Story) -> Grammar {
        let lx = &story.lexicon;
        let mut phrases: Vec<(Vec<String>, Vec<String>)> =
            lx.phrases.iter().map(|(k, v)| (words(k), words(v))).collect();
        phrases.sort_by_key(|(k, _)| std::cmp::Reverse(k.len()));
        let mut filler: Vec<Vec<String>> = lx.filler.iter().map(|f| words(f)).collect();
        filler.sort_by_key(|f| std::cmp::Reverse(f.len()));

        let mut verb_words = HashSet::new();
        let mut pattern_words = HashSet::new();
        for v in &story.verbs {
            for p in &v.patterns {
                if let Some(PatternToken::Word(w)) = p.0.first() {
                    verb_words.insert(w.clone());
                }
                for t in &p.0 {
                    if let PatternToken::Word(w) = t {
                        pattern_words.insert(w.clone());
                    }
                }
            }
        }
        let vocab_words = story
            .entities
            .iter()
            .flat_map(|e| e.vocab.nouns.iter().chain(&e.vocab.adjectives))
            .cloned()
            .collect();

        let lang = lang::for_language(&story.meta.language).unwrap_or_else(|| lang::for_language("en").unwrap());
        let skip: HashSet<&str> = lx
            .stopwords
            .iter()
            .chain(&lx.articles)
            .chain(&lx.pronouns)
            .chain(&lx.all_words)
            .chain(&lx.self_words)
            .chain(lx.directions.keys())
            .map(String::as_str)
            .collect();
        let described: Vec<HashSet<String>> = story
            .entities
            .iter()
            .map(|e| {
                let mut words = Vec::new();
                if let Some(d) = &e.description {
                    text_words(d, &mut words);
                }
                let mut set = HashSet::new();
                for w in words {
                    // Very short words ("ox" aside) are almost never what a player means.
                    if w.chars().count() < 3 || skip.contains(w.as_str()) || w.chars().all(|c| c.is_ascii_digit()) {
                        continue;
                    }
                    set.extend((lang.stems)(&w));
                    set.insert(w);
                }
                set
            })
            .collect();
        let described_words = described.iter().flatten().cloned().collect();

        Grammar {
            lang,
            phrases,
            filler,
            articles: lx.articles.iter().cloned().collect(),
            pronouns: lx.pronouns.iter().cloned().collect(),
            all_words: lx.all_words.iter().cloned().collect(),
            directions: lx.directions.iter().map(|(k, v)| (k.clone(), *v)).collect(),
            verb_words,
            vocab_words,
            pattern_words,
            described,
            described_words,
            corpse_words: lx.corpse_words.iter().cloned().collect(),
            and_words: lx.and_words.iter().cloned().collect(),
            except_words: lx.except_words.iter().cloned().collect(),
            room_words: lx.room_words.iter().cloned().collect(),
            my_words: lx.my_words.iter().cloned().collect(),
            one_words: lx.one_words.iter().cloned().collect(),
            other_words: lx.other_words.iter().cloned().collect(),
            ordinals: lx.ordinals.iter().map(|(k, v)| (k.clone(), *v)).collect(),
            numbers: lx.numbers.iter().map(|(k, v)| (k.clone(), *v)).collect(),
            adverbs: lx.adverbs.iter().cloned().collect(),
            trailing: {
                let mut t: Vec<Vec<String>> = lx.trailing.iter().map(|f| words(f)).collect();
                t.sort_by_key(|f| std::cmp::Reverse(f.len()));
                t
            },
            aliases: {
                let mut a: Vec<(Vec<String>, Vec<String>)> = lx.aliases.iter().map(|(k, v)| (words(k), words(v))).collect();
                a.sort_by_key(|(k, _)| std::cmp::Reverse(k.len()));
                a
            },
        }
    }

    /// The game's aliases swapped in, wherever they appear.
    fn apply_aliases(&self, words: Vec<String>) -> Vec<String> {
        if self.aliases.is_empty() {
            return words;
        }
        let mut out = Vec::new();
        let mut i = 0;
        'outer: while i < words.len() {
            for (from, to) in &self.aliases {
                if words[i..].starts_with(from) {
                    out.extend(to.iter().cloned());
                    i += from.len();
                    continue 'outer;
                }
            }
            out.push(words[i].clone());
            i += 1;
        }
        out
    }

    pub fn lang(&self) -> &'static Lang {
        self.lang
    }

    /// Splits raw input into separate commands, each one a list of
    /// lowercase words. "." and ";" separate commands, and so does "then",
    /// and so does "and" when a command follows it ("take the key and
    /// open the door", but not "take the key and the lamp").
    pub fn split_commands(&self, input: &str) -> Vec<Vec<String>> {
        let cleaned: String = input
            .to_lowercase()
            .chars()
            .flat_map(|c| match c {
                '.' | ';' | '!' | '?' => vec!['.'],
                // A comma lists things, like "and": "the key, the coin and the lamp".
                ',' => vec![' ', ',', ' '],
                c if c.is_alphanumeric() || c == '\'' || c == '-' || c == ENTITY_TOKEN => vec![c],
                _ => vec![' '],
            })
            .collect();
        let and = self.and_words.iter().min().cloned();
        let mut out = Vec::new();
        for part in cleaned.split('.') {
            let mut cur = Vec::new();
            let words: Vec<String> = part
                .split_whitespace()
                .filter_map(|w| if w == "," { and.clone() } else { Some(w.to_string()) })
                .collect();
            let words = self.apply_aliases(words);
            // UNDO UNDO: the same command twice.
            if words.len() > 1 && words.iter().all(|w| *w == words[0]) && self.verb_words.contains(&words[0]) {
                out.extend(words.iter().map(|w| vec![w.clone()]));
                continue;
            }
            for (i, w) in words.iter().enumerate() {
                let w = w.as_str();
                let next = words.get(i + 1).map(String::as_str);
                // A verb (or direction) after "and" starts a new command,
                // unless it's also the name of something here: "the hammer and saw".
                let starts_command = |n: &str| {
                    n == "then"
                        || (!self.vocab_words.contains(n)
                            && (self.is_known_start(n)
                                || (self.lang.stems)(n).iter().any(|s| self.is_known_start(s) && !self.directions.contains_key(s))))
                };
                // A stray "and" (", and", or at either end) joins nothing.
                if self.and_words.contains(w) && (cur.is_empty() || next.is_none_or(|n| self.and_words.contains(n))) {
                    continue;
                }
                if self.and_words.contains(w) && !cur.is_empty() && next.is_some_and(starts_command) {
                    out.push(std::mem::take(&mut cur));
                } else if w == "then" {
                    if !cur.is_empty() {
                        out.push(std::mem::take(&mut cur));
                    }
                } else {
                    cur.push(w.to_string());
                }
            }
            if !cur.is_empty() {
                out.push(cur);
            }
        }
        out
    }

    /// Verbs (and directions) a word that isn't one might have meant, for
    /// "Did you mean…?". None for a word that names something.
    pub fn verb_guesses(&self, word: &str) -> Vec<String> {
        let names = |w: &str| self.vocab_words.contains(w) || self.corpse_words.contains(w);
        if names(word) || (self.lang.stems)(word).iter().any(|s| names(s)) {
            return Vec::new();
        }
        let mut starts: Vec<String> =
            self.verb_words.iter().chain(self.directions.keys()).chain(self.phrases.iter().map(|(k, _)| &k[0])).cloned().collect();
        starts.sort();
        starts.dedup();
        near_misses(word, &starts)
    }

    /// Names of things here a word might have meant.
    pub fn noun_guesses(&self, world: &World, word: &str) -> Vec<String> {
        let vocab: Vec<String> = world
            .scope()
            .into_iter()
            .flat_map(|e| {
                let v = &world.def(e).vocab;
                v.nouns.iter().chain(&v.adjectives).cloned().collect::<Vec<_>>()
            })
            .collect();
        near_misses(word, &vocab)
    }

    /// AGAIN WITH THE IRON KEY: the last command's words with a different
    /// object, as the readings worth trying, best first. After a linking
    /// word it's the second object ("unlock door with brass key", "again
    /// with the iron key"), or a new way of putting it ("put coin in box",
    /// "again on the table"), or added ("unlock door", "again with the
    /// key"), or else the first object ("put coin in box", "again with the
    /// ring"). Without one, it's the first object.
    pub fn swap_object(&self, world: &World, last: &[String], new: &[String]) -> Vec<Vec<String>> {
        let mut cands = self.candidates(world.story(), last);
        cands.sort_by_key(|c| std::cmp::Reverse(c.specificity));
        // The reading that made sense then; the thing may be gone by now.
        let ok = cands.iter().position(|c| self.resolve_candidate(world, last, c).is_ok()).unwrap_or(0);
        let Some(c) = cands.get(ok) else { return Vec::new() };
        let splice = |(s, e): (usize, usize), words: &[String]| -> Vec<String> {
            last[..s].iter().chain(words).chain(&last[e..]).cloned().collect()
        };
        let link = |w: &String| self.pattern_words.contains(w) && !self.vocab_words.contains(w) && !self.articles.contains(w);
        let mut out = Vec::new();
        let rest = match new.first().filter(|w| link(w)) {
            Some(w) => {
                let rest = &new[1..];
                match c.caps.tool {
                    Some((s, e)) if s > 0 && last[s - 1] == *w => out.push(splice((s, e), rest)),
                    Some((s, e)) if s > 0 && link(&last[s - 1]) => out.push(splice((s - 1, e), new)),
                    Some(span) => out.push(splice(span, rest)),
                    None => out.push(last.iter().chain(new).cloned().collect()),
                }
                rest
            }
            None => new,
        };
        if let Some(span) = c.caps.target.or(c.caps.tool)
            && !rest.is_empty()
        {
            out.push(splice(span, rest));
        }
        out.retain(|w| w.as_slice() != last);
        out
    }

    fn is_known_start(&self, w: &str) -> bool {
        self.verb_words.contains(w) || self.directions.contains_key(w) || self.phrases.iter().any(|(k, _)| k[0] == w)
    }

    /// Normalization: drop leading filler, give inflected verbs their base
    /// form, and rewrite synonym phrases at the start of the command.
    pub fn normalize(&self, mut toks: Vec<String>) -> Vec<String> {
        while let Some(f) = self.filler.iter().find(|f| toks.len() > f.len() && toks.starts_with(f)) {
            toks.drain(..f.len());
        }
        // "look closely at the lamp": adverbs change nothing (unless they're a
        // name, or the command itself: AGAIN WITH THE KEY).
        if toks.len() > 1 {
            let kept: Vec<String> = toks
                .iter()
                .enumerate()
                .filter(|(i, w)| !self.adverbs.contains(*w) || self.vocab_words.contains(*w) || (*i == 0 && self.verb_words.contains(*w)))
                .map(|(_, w)| w.clone())
                .collect();
            if !kept.is_empty() {
                toks = kept;
            }
        }
        if let Some(first) = toks.first()
            && !self.is_known_start(first)
            // "sing" isn't "s" (south), and "exits" isn't "exit".
            && let Some(stem) = (self.lang.stems)(first).into_iter().find(|s| self.is_known_start(s) && !self.directions.contains_key(s))
        {
            toks[0] = stem;
        }
        if let Some((from, to)) = self.phrases.iter().find(|(k, _)| toks.starts_with(k)) {
            toks.splice(..from.len(), to.iter().cloned());
        }
        toks
    }

    fn candidates(&self, story: &Story, toks: &[String]) -> Vec<Candidate> {
        let mut out = Vec::new();
        for (vi, verb) in story.verbs.iter().enumerate() {
            for pattern in &verb.patterns {
                let mut found = Vec::new();
                self.match_tokens(&pattern.0, toks, 0, Caps::default(), &mut found);
                let specificity = pattern.0.iter().filter(|t| matches!(t, PatternToken::Word(_))).count();
                out.extend(found.into_iter().map(|caps| Candidate { verb: vi, caps, specificity }));
            }
        }
        out
    }

    fn match_tokens(&self, pat: &[PatternToken], toks: &[String], pos: usize, caps: Caps, out: &mut Vec<Caps>) {
        let Some(first) = pat.first() else {
            if pos == toks.len() {
                out.push(caps);
            }
            return;
        };
        match first {
            PatternToken::Word(w) => {
                if toks.get(pos) == Some(w) {
                    self.match_tokens(&pat[1..], toks, pos + 1, caps, out);
                }
            }
            PatternToken::Slot(Slot::Direction) => {
                if let Some(d) = toks.get(pos).and_then(|t| self.directions.get(t)) {
                    let mut c = caps;
                    c.direction = Some(*d);
                    self.match_tokens(&pat[1..], toks, pos + 1, c, out);
                }
            }
            PatternToken::Slot(slot) => {
                // Followed straight by {text}: try the longest object first,
                // so "call the old dog fido" names "the old dog".
                let mut ends: Vec<usize> = (pos + 1..=toks.len()).collect();
                if matches!(pat.get(1), Some(PatternToken::Slot(Slot::Text))) {
                    ends.reverse();
                }
                for end in ends {
                    let mut c = caps.clone();
                    let span = Some((pos, end));
                    match slot {
                        Slot::Target => c.target = span,
                        Slot::Tool => c.tool = span,
                        Slot::Text => c.text = span,
                        Slot::Direction => unreachable!(),
                    }
                    self.match_tokens(&pat[1..], toks, end, c, out);
                }
            }
        }
    }

    pub fn parse(&self, world: &World, toks: &[String]) -> Parsed {
        let parsed = self.parse_words(world, toks);
        // GO {text} takes anything; it only counts if it names a place.
        let nowhere = |p: &Parsed| match p {
            Parsed::Command(c) => {
                world.story().verbs[c.verb].action == VerbAction::Builtin(BuiltinAction::GoTo)
                    && world.places_named(&c.text.as_deref().unwrap_or("").split_whitespace().filter(|w| !self.articles.contains(*w)).map(str::to_string).collect::<Vec<_>>(), &[]).is_empty()
            }
            _ => true,
        };
        if !nowhere(&parsed) {
            return parsed;
        }
        // "save game", "go east please": try again without the extra words at the end.
        for t in &self.trailing {
            if toks.len() > t.len() && toks.ends_with(t) {
                let fewer = &toks[..toks.len() - t.len()];
                if let p @ Parsed::Command(_) = self.parse(world, fewer) {
                    return p;
                }
            }
        }
        parsed
    }

    fn parse_words(&self, world: &World, toks: &[String]) -> Parsed {
        if toks.is_empty() {
            return Parsed::Fail(Failure::Empty);
        }
        let story = world.story();
        let toks = toks.to_vec();
        let mut cands = self.candidates(story, &toks);

        if cands.is_empty() {
            return self.no_pattern(world, &toks);
        }
        // The pattern with the most fixed words first: "wait until {text}"
        // before "wait {text}", "ask {target} for forgiveness" before
        // "ask {target} for {text}".
        cands.sort_by_key(|c| std::cmp::Reverse(c.specificity));

        let mut best: Option<(usize, Parsed)> = None;
        for c in &cands {
            match self.resolve_candidate(world, &toks, c) {
                Ok(cmd) => return Parsed::Command(cmd),
                Err(p) => {
                    if best.as_ref().is_none_or(|(s, _)| c.specificity > *s) {
                        best = Some((c.specificity, p));
                    }
                }
            }
        }
        best.map(|(_, p)| p).unwrap_or(Parsed::Fail(Failure::NotHere))
    }

/// The verb's name for questions: the leading words of its first pattern
    /// ("x" → "examine").
    fn verb_name(verb: &eifgb_ir::Verb) -> String {
        // The first pattern that takes something: THANK {target}, not THANKS.
        let pattern = verb.patterns.iter().find(|p| p.0.iter().any(|t| !matches!(t, PatternToken::Word(_)))).or(verb.patterns.first());
        let lead: Vec<&str> = pattern
            .map(|p| {
                p.0.iter()
                    .map_while(|t| match t {
                        PatternToken::Word(w) => Some(w.as_str()),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        lead.join(" ")
    }

    /// The verb as the player put it, for "What do you want to …?": the
    /// words they typed before what's missing (SHAKE, not WAKE), or else,
    /// for nothing or a short form (X, L AT), the verb's own.
    fn asked_verb(verb: &eifgb_ir::Verb, typed: &[String]) -> String {
        if typed.first().is_none_or(|w| w.chars().count() < 3) {
            Self::verb_name(verb)
        } else {
            typed.join(" ")
        }
    }

    /// No pattern fits. The input might be a verb waiting for its
    /// object, a verb and object waiting for a second object, a known
    /// verb with leftover words, or a bare noun.
    fn no_pattern(&self, world: &World, toks: &[String]) -> Parsed {
        let story = world.story();
        for verb in &story.verbs {
            for p in &verb.patterns {
                let lead: Vec<&String> = p
                    .0
                    .iter()
                    .map_while(|t| match t {
                        PatternToken::Word(w) => Some(w),
                        _ => None,
                    })
                    .collect();
                if lead.len() == toks.len()
                    && lead.iter().zip(toks).all(|(a, b)| *a == b)
                    && matches!(p.0.get(lead.len()), Some(PatternToken::Slot(_)))
                {
                    let way = matches!(p.0.get(lead.len()), Some(PatternToken::Slot(Slot::Direction)));
                    return Parsed::NeedNoun { prefix: toks.to_vec(), verb: Self::asked_verb(verb, toks), target: None, word: None, way };
                }
            }
        }
        // "fill lamp" against "fill {target} with {tool}": ask for the tool.
        for (vi, verb) in story.verbs.iter().enumerate() {
            for p in &verb.patterns {
                let n = p.0.len();
                let (Some(PatternToken::Word(link)), Some(PatternToken::Slot(Slot::Tool))) =
                    (n.checked_sub(2).and_then(|i| p.0.get(i)), p.0.last())
                else {
                    continue;
                };
                let mut found = Vec::new();
                self.match_tokens(&p.0[..n - 2], toks, 0, Caps::default(), &mut found);
                for caps in found {
                    let c = Candidate { verb: vi, caps, specificity: 0 };
                    if let Ok(cmd) = self.resolve_candidate(world, toks, &c)
                        && let [target] = cmd.targets[..]
                    {
                        let mut prefix = toks.to_vec();
                        prefix.push(link.clone());
                        return Parsed::NeedNoun {
                            prefix,
                            verb: Self::verb_name(verb),
                            target: Some(target),
                            word: Some(link.clone()),
                            way: false,
                        };
                    }
                }
            }
        }
        if self.verb_words.contains(&toks[0]) {
            return Parsed::Fail(Failure::Partial { verb: toks[0].clone() });
        }
        // A bare noun means "examine it".
        if let Ok(ids) = self.resolve(world, toks, None, None)
            && ids.len() == 1
            && let Some(vi) = story.verbs.iter().position(|v| v.action == VerbAction::Builtin(BuiltinAction::Examine))
        {
            return Parsed::Command(Command {
                verb: vi,
                targets: ids,
                tool: None,
                text: None,
                direction: None,
                words: toks.to_vec(),
            });
        }
        // A place's name: KITCHEN means GO TO THE KITCHEN.
        let place: Vec<String> = toks.iter().filter(|w| !self.articles.contains(*w)).cloned().collect();
        if !world.places_named(&place, &[]).is_empty()
            && let Some(vi) = story.verbs.iter().position(|v| v.action == VerbAction::Builtin(BuiltinAction::GoTo))
        {
            return Parsed::Command(Command { verb: vi, targets: Vec::new(), tool: None, text: Some(toks.join(" ")), direction: None, words: toks.to_vec() });
        }
        Parsed::Fail(Failure::UnknownVerb { word: toks[0].clone() })
    }

    fn resolve_candidate(&self, world: &World, toks: &[String], c: &Candidate) -> Result<Command, Parsed> {
        let verb = &world.story().verbs[c.verb];
        let action = match verb.action {
            VerbAction::Builtin(a) => Some(a),
            VerbAction::Script => None,
        };
        let fail = |nf: NounFail, span: (usize, usize)| -> Parsed {
            match nf {
                NounFail::Missing => Parsed::NeedNoun {
                    prefix: toks[..span.0].to_vec(),
                    verb: Self::asked_verb(verb, &toks[..span.0]),
                    target: None,
                    word: None,
                    way: false,
                },
                NounFail::NoPronoun => Parsed::Fail(Failure::NoPronoun),
                NounFail::NotFound { unknown_word: Some(w) } => Parsed::Fail(Failure::UnknownWord { word: w }),
                NounFail::NotFound { unknown_word: None } => Parsed::Fail(Failure::NotHere),
                NounFail::Scenery => Parsed::Fail(Failure::Scenery),
                NounFail::Structural => Parsed::Fail(Failure::Partial { verb: Self::asked_verb(verb, &toks[..span.0]) }),
                NounFail::Ambiguous(options) => Parsed::Ambiguous {
                    before: toks[..span.0].to_vec(),
                    after: toks[span.1..].to_vec(),
                    options,
                },
                NounFail::NoAll => Parsed::Fail(Failure::NoAll),
                NounFail::AllNone => Parsed::Fail(Failure::AllNone),
            }
        };

        // The second object first: "take all from the bag" and "put all in
        // the jar" need it to know what "all" means.
        let tool = match c.caps.tool {
            Some(span) => {
                // TRADE X FOR Y: Y is something a merchant is selling.
                let for_sale = action.filter(|a| *a == BuiltinAction::Trade);
                let ids = self.resolve(world, &toks[span.0..span.1], for_sale, None).map_err(|e| fail(e, span))?;
                if ids.len() != 1 {
                    return Err(Parsed::Fail(Failure::NoAll));
                }
                Some(ids[0])
            }
            None => None,
        };
        let targets = match c.caps.target {
            Some(span) => self.resolve(world, &toks[span.0..span.1], action, tool).map_err(|e| fail(e, span))?,
            None => Vec::new(),
        };
        let text = c.caps.text.map(|(s, e)| toks[s..e].join(" "));
        Ok(Command { verb: c.verb, targets, tool, text, direction: c.caps.direction, words: toks.to_vec() })
    }

    /// Resolves a noun phrase against what's in scope.
    fn resolve(
        &self,
        world: &World,
        phrase: &[String],
        action: Option<BuiltinAction>,
        tool: Option<EntityId>,
    ) -> Result<Vec<EntityId>, NounFail> {
        let words: Vec<&String> = phrase.iter().filter(|w| !self.articles.contains(*w)).collect();
        if words.is_empty() {
            return Err(NounFail::Missing);
        }
        let mut scope = world.scope();
        // Stealing (or TAKE … FROM someone): what they're carrying, in plain sight.
        let victims: Vec<EntityId> = match (action, tool) {
            (Some(BuiltinAction::Take | BuiltinAction::Steal), Some(c)) if world.def(c).kind == eifgb_ir::EntityKind::Character => vec![c],
            (Some(BuiltinAction::Steal), None) => scope.iter().copied().filter(|&c| world.def(c).kind == eifgb_ir::EntityKind::Character && c != PLAYER).collect(),
            _ => Vec::new(),
        };
        for v in victims {
            scope.extend(world.contents(v).into_iter().filter(|&i| !world.is_hidden(i)));
        }
        // What merchants here are selling can be looked at, bought and traded for.
        if matches!(action, Some(BuiltinAction::Buy | BuiltinAction::Examine | BuiltinAction::Offer | BuiltinAction::Trade)) {
            for m in world.merchants_here() {
                scope.extend(world.contents(m).into_iter().filter(|&i| world.def(i).price.is_some()));
            }
        }

        // "all but the key (and the lamp)".
        if let Some(i) = phrase.iter().position(|w| self.except_words.contains(w)) {
            let all: Vec<&String> = phrase[..i].iter().filter(|w| !self.articles.contains(*w)).collect();
            if all.len() == 1 && self.all_words.contains(all[0]) {
                let items = self.resolve_all(world, &scope, action, tool)?;
                let mut left_out = Vec::new();
                for part in phrase[i + 1..].split(|w| self.and_words.contains(w)) {
                    let part: Vec<&String> = part.iter().filter(|w| !self.articles.contains(*w)).collect();
                    if !part.is_empty() {
                        left_out.extend(self.match_scope(world, &items, &part, true));
                    }
                }
                let rest: Vec<EntityId> = items.into_iter().filter(|id| !left_out.contains(id)).collect();
                return if rest.is_empty() { Err(NounFail::AllNone) } else { Ok(rest) };
            }
        }
        // "the key and the lamp": each of them, unless one thing is called that.
        if words.iter().any(|w| self.and_words.contains(*w)) && self.match_scope(world, &scope, &words, true).is_empty() {
            let mut out = Vec::new();
            for part in phrase.split(|w| self.and_words.contains(w)) {
                if part.iter().all(|w| self.articles.contains(w)) {
                    continue;
                }
                for id in self.resolve(world, part, action, tool)? {
                    if !out.contains(&id) {
                        out.push(id);
                    }
                }
            }
            if !out.is_empty() {
                return Ok(out);
            }
        }

        if words.len() == 1 {
            let w = words[0];
            if let Some(n) = w.strip_prefix(ENTITY_TOKEN).and_then(|n| n.parse::<u32>().ok()) {
                let id = EntityId(n);
                return if scope.contains(&id) { Ok(vec![id]) } else { Err(NounFail::NotFound { unknown_word: None }) };
            }
            if self.pronouns.contains(w) {
                return match world.state.mentioned.first() {
                    Some(id) if scope.contains(id) => Ok(vec![*id]),
                    Some(_) => Err(NounFail::NotFound { unknown_word: None }),
                    None => Err(NounFail::NoPronoun),
                };
            }
            if self.all_words.contains(w) {
                return self.resolve_all(world, &scope, action, tool);
            }
        }

        let mut matches = self.match_scope(world, &scope, &words, true);
        // The room itself: "x room", "search here", "look at the hall".
        if matches.is_empty()
            && matches!(action, Some(BuiltinAction::Examine | BuiltinAction::Search))
            && !world.in_darkness()
        {
            let here = world.here();
            let name = words_of(&world.def(here).name);
            if words.iter().all(|w| self.room_words.contains(*w) || name.contains(*w)) {
                return Ok(vec![here]);
            }
        }
        // "take key from the floor": the floor and the ground are the room, for any verb.
        if matches.is_empty() && !world.in_darkness() && words.iter().all(|w| self.room_words.contains(*w)) && matches!(words.last().map(|w| w.as_str()), Some("floor" | "ground")) {
            return Ok(vec![world.here()]);
        }
        if matches.is_empty()
            && let Some(r) = self.resolve_modified(world, &scope, &words, action)
        {
            return r;
        }
        if matches.is_empty() {
            // Step 4: words the player read in a description: "x glint" after "Something
            // glints at the bottom." Real names always win; this is only a fallback.
            matches = self.match_descriptions(world, &scope, &words);
            if matches.is_empty() && words.iter().all(|w| self.describes(world.here(), w)) {
                return Err(NounFail::Scenery);
            }
        }
        match matches.len() {
            0 if words.iter().any(|w| self.pattern_words.contains(*w) && !self.vocab_words.contains(*w)) => {
                Err(NounFail::Structural)
            }
            0 => {
                // A name the player gave something elsewhere isn't unknown.
                let named = |w: &str| (0..world.story().entities.len() as u32).any(|i| world.named_word(EntityId(i), w));
                let unknown = words
                    .iter()
                    .find(|w| !self.vocab_words.contains(**w) && !self.stem_known(w) && !named(w) && self.direction_of(w).is_none() && !self.is_modifier(w))
                    .map(|w| (*w).clone());
                Err(NounFail::NotFound { unknown_word: unknown })
            }
            1 => Ok(matches),
            _ => self.one_of(world, &matches, action),
        }
    }

    /// "my", "one", "other", "second", "two": words that pick among things rather than name them.
    fn is_modifier(&self, w: &str) -> bool {
        self.my_words.contains(w)
            || self.one_words.contains(w)
            || self.other_words.contains(w)
            || self.ordinals.contains_key(w)
            || self.numbers.contains_key(w)
            || w.parse::<u32>().is_ok()
    }

    /// "my hat", "the brass one", "the other key", "the second key", "two
    /// coins". None when there's no such word, or the rest names nothing.
    fn resolve_modified(
        &self,
        world: &World,
        scope: &[EntityId],
        words: &[&String],
        action: Option<BuiltinAction>,
    ) -> Option<Result<Vec<EntityId>, NounFail>> {
        if !words.iter().any(|w| self.is_modifier(w)) {
            return None;
        }
        let has = |set: &HashSet<String>| words.iter().any(|w| set.contains(*w));
        let (mine, other, one) = (has(&self.my_words), has(&self.other_words), has(&self.one_words));
        let ordinal = words.iter().find_map(|w| self.ordinals.get(*w).copied());
        let count = words
            .iter()
            .filter(|w| !self.one_words.contains(**w))
            .find_map(|w| self.numbers.get(*w).copied().or_else(|| w.parse::<u32>().ok()));
        let rest: Vec<&String> = words.iter().copied().filter(|w| !self.is_modifier(w)).collect();
        // They come before the name ("the second key", "two coins"), except
        // "one" at the end ("the brass one"); a bare number names nothing.
        let first_name = words.iter().position(|w| !self.is_modifier(w)).unwrap_or(words.len());
        let misplaced = words.iter().enumerate().any(|(i, w)| {
            i > first_name && self.is_modifier(w) && !(self.one_words.contains(*w) && i == words.len() - 1)
        });
        if misplaced || (rest.is_empty() && !one && !other) {
            return None;
        }
        let things = |id: &EntityId| *id != PLAYER && world.def(*id).kind != eifgb_ir::EntityKind::Room;
        let mut found: Vec<EntityId> = if !rest.is_empty() {
            let m = self.match_scope(world, scope, &rest, true);
            // "the brass one" stands in for the noun; "the dog one" isn't English.
            let last = rest[rest.len() - 1];
            if m.is_empty() || (one && m.iter().any(|id| world.def(*id).vocab.nouns.contains(last))) {
                return None;
            }
            m
        } else if one || other || ordinal.is_some() || count.is_some() {
            // "the other one": things like the one just mentioned.
            let Some(last) = world.state.mentioned.first() else { return Some(Err(NounFail::Missing)) };
            let nouns = &world.def(*last).vocab.nouns;
            scope.iter().copied().filter(things).filter(|id| world.def(*id).vocab.nouns.iter().any(|n| nouns.contains(n))).collect()
        } else if mine {
            world.inventory()
        } else {
            return Some(Err(NounFail::Missing));
        };
        if other {
            found.retain(|id| !world.state.mentioned.contains(id));
        }
        if mine {
            let held: Vec<EntityId> = found.iter().copied().filter(|id| world.is_within(*id, PLAYER)).collect();
            if !held.is_empty() {
                found = held;
            }
        }
        if let Some(n) = ordinal {
            let pick = if n == 0 { found.last() } else { found.get(n as usize - 1) };
            return Some(pick.map(|id| vec![*id]).ok_or(NounFail::NotFound { unknown_word: None }));
        }
        if let Some(n) = count
            && n > 1
        {
            found.truncate(n as usize);
            return Some(if found.is_empty() { Err(NounFail::NotFound { unknown_word: None }) } else { Ok(found) });
        }
        Some(match found.len() {
            0 => Err(NounFail::NotFound { unknown_word: None }),
            1 => Ok(found),
            _ => self.one_of(world, &found, action),
        })
    }

    fn stem_known(&self, w: &str) -> bool {
        self.corpse_words.contains(w)
            || (self.lang.stems)(w).iter().any(|s| self.vocab_words.contains(s) || self.corpse_words.contains(s))
            || self.described_words.contains(w)
            || (self.lang.stems)(w).iter().any(|s| self.described_words.contains(s))
    }

    /// Whether a command can start with this word.
    pub fn is_verb_word(&self, w: &str) -> bool {
        self.verb_words.contains(w)
    }

    /// Whether the room the player is in describes the word (it's scenery).
    pub fn describes_here(&self, world: &World, w: &str) -> bool {
        !self.articles.contains(w) && self.describes(world.here(), w)
    }

    /// Whether `id`'s description uses the word, or another form of it.
    fn describes(&self, id: EntityId, w: &str) -> bool {
        let set = &self.described[id.index()];
        set.contains(w) || (self.lang.stems)(w).iter().any(|s| set.contains(s))
    }

    /// Things in scope (not rooms) where every word is either in their
    /// vocabulary or in their description, using at least one description word.
    fn match_descriptions(&self, world: &World, scope: &[EntityId], words: &[&String]) -> Vec<EntityId> {
        scope
            .iter()
            .copied()
            .filter(|&id| world.def(id).kind != eifgb_ir::EntityKind::Room)
            .filter(|&id| {
                let v = &world.def(id).vocab;
                let named = |w: &str| {
                    let check = |s: &str| v.nouns.iter().any(|n| n == s) || v.adjectives.iter().any(|a| a == s);
                    check(w) || (self.lang.stems)(w).iter().any(|s| check(s))
                };
                words.iter().all(|w| named(w) || self.describes(id, w)) && words.iter().any(|w| self.describes(id, w))
            })
            .collect()
    }

    /// Entities in scope whose vocabulary covers every word. Matches that
    /// end on a noun beat matches made only of adjectives.
    /// The direction a word means ("n", "north", "up"), if any.
    pub fn direction_of(&self, w: &str) -> Option<Direction> {
        self.directions.get(w).copied()
    }

    /// Things in scope that a noun phrase ("the old dog") names. For orders
    /// and "REX, SIT", outside the usual command patterns.
    pub fn name_matches(&self, world: &World, phrase: &[String]) -> Vec<EntityId> {
        self.name_matches_in(world, phrase, &world.scope())
    }

    /// Like [`Grammar::name_matches`], among the given entities.
    pub fn name_matches_in(&self, world: &World, phrase: &[String], among: &[EntityId]) -> Vec<EntityId> {
        let words: Vec<&String> = phrase.iter().filter(|w| !self.articles.contains(*w)).collect();
        if words.is_empty() {
            return Vec::new();
        }
        self.match_scope(world, among, &words, true)
    }

    /// Like [`Grammar::name_matches_in`], for things out of sight: by the
    /// names they always have, and the ones the player gave them, not by
    /// how they are now ("the body", "the water"), which the player can't
    /// know.
    pub fn remembered_matches(&self, world: &World, phrase: &[String], among: &[EntityId]) -> Vec<EntityId> {
        let words: Vec<&String> = phrase.iter().filter(|w| !self.articles.contains(*w)).collect();
        if words.is_empty() {
            return Vec::new();
        }
        self.match_scope(world, among, &words, false)
    }

    /// `live`: also by how things are now (dead, full of water, the door to
    /// the north from here).
    fn match_scope(&self, world: &World, scope: &[EntityId], words: &[&String], live: bool) -> Vec<EntityId> {
        let mut best_score = 0;
        let mut out = Vec::new();
        for &id in scope {
            let v = &world.def(id).vocab;
            let dead = live && world.is_dead(id);
            // "corpse" for the dead; a name the player gave it, as a noun.
            let corpse_word = |s: &str| (dead && self.corpse_words.contains(s)) || world.named_word(id, s);
            // "the north door": a door named by the way it leads from here.
            let door_way = |s: &str| live && self.direction_of(s).is_some_and(|d| world.door_toward(world.here(), d) == Some(id));
            // "the water": a bottle with water in it.
            let liquid = |s: &str| live && world.def(id).holds_liquid && matches!(world.get(id, "liquid"), Some(eifgb_ir::Value::Str(l)) if l.split_whitespace().any(|w| w == s));
            let known = |w: &str| {
                let check = |s: &str| {
                    v.nouns.iter().any(|n| n == s) || v.adjectives.iter().any(|a| a == s) || corpse_word(s) || door_way(s) || liquid(s)
                };
                check(w) || (self.lang.stems)(w).iter().any(|s| check(s))
            };
            if (v.is_empty() && !world.def(id).holds_liquid) || !words.iter().all(|w| known(w)) {
                continue;
            }
            let last = words[words.len() - 1];
            let noun = |s: &str| v.nouns.iter().any(|n| n == s) || corpse_word(s) || liquid(s);
            let noun_last = noun(last) || (self.lang.stems)(last).iter().any(|s| noun(s));
            let score = if noun_last { 2 } else { 1 };
            if score > best_score {
                best_score = score;
                out.clear();
            }
            if score == best_score {
                out.push(id);
            }
        }
        out
    }

    /// Several things match: narrow them down, or ask which. Things that
    /// can't be told apart (two gold coins) don't need asking about.
    fn one_of(&self, world: &World, matches: &[EntityId], action: Option<BuiltinAction>) -> Result<Vec<EntityId>, NounFail> {
        let narrowed = self.prefer(world, matches, action);
        let name = |id: &EntityId| world.display_name(*id);
        let interchangeable = |id: &EntityId| world.has(*id, Trait::Portable) && !world.has(*id, Trait::Door) && name(id) == name(&narrowed[0]);
        if narrowed.len() == 1 || narrowed.iter().all(interchangeable) {
            Ok(vec![narrowed[0]])
        } else {
            Err(NounFail::Ambiguous(narrowed))
        }
    }

    /// Narrows several matches using what makes sense for the verb, e.g.
    /// TAKE prefers things you aren't holding.
    fn prefer(&self, world: &World, matches: &[EntityId], action: Option<BuiltinAction>) -> Vec<EntityId> {
        let held = |id: &EntityId| world.is_within(*id, PLAYER);
        let filtered: Vec<EntityId> = match action {
            Some(BuiltinAction::Take) => matches.iter().filter(|id| !held(id)).copied().collect(),
            Some(
                BuiltinAction::Drop
                | BuiltinAction::PutIn
                | BuiltinAction::PutOn
                | BuiltinAction::Wear
                | BuiltinAction::Eat,
            ) => matches.iter().filter(|id| held(id)).copied().collect(),
            Some(BuiltinAction::TakeOff) => matches.iter().filter(|id| world.flag(**id, "worn")).copied().collect(),
            _ => Vec::new(),
        };
        let filtered = if filtered.is_empty() { matches.to_vec() } else { filtered };
        // "the water": what's in the bottle in hand, rather than the stream it came from.
        let poured: Vec<EntityId> = filtered.iter().copied().filter(|&id| world.def(id).holds_liquid && held(&id)).collect();
        if !poured.is_empty() && filtered.iter().all(|&id| poured.contains(&id) || world.def(id).liquid_source.is_some()) {
            return poured;
        }
        filtered
    }

    /// What "all" means for a verb. With a second object, TAKE ALL FROM X
    /// means what's in X, and PUT ALL IN X means what you carry except X.
    fn resolve_all(
        &self,
        world: &World,
        scope: &[EntityId],
        action: Option<BuiltinAction>,
        tool: Option<EntityId>,
    ) -> Result<Vec<EntityId>, NounFail> {
        let portable = |id: EntityId| id != PLAYER && world.has(id, Trait::Portable);
        let items: Vec<EntityId> = match (action, tool) {
            (Some(BuiltinAction::Take), Some(from)) => {
                world.contents(from).into_iter().filter(|id| portable(*id) && scope.contains(id)).collect()
            }
            (Some(BuiltinAction::Take), None) => {
                scope.iter().copied().filter(|&id| portable(id) && !world.is_within(id, PLAYER)).collect()
            }
            (Some(BuiltinAction::Drop), _) => world.inventory(),
            // X ALL: everything in sight, one after another.
            (Some(BuiltinAction::Examine), None) => scope
                .iter()
                .copied()
                .filter(|&id| id != PLAYER && world.def(id).kind != eifgb_ir::EntityKind::Room && !world.has(id, Trait::Scenery) && !world.is_hidden(id))
                .collect(),
            (Some(BuiltinAction::PutIn | BuiltinAction::PutOn), Some(dest)) => world
                .inventory()
                .into_iter()
                .filter(|&id| id != dest && !world.is_within(dest, id) && !world.flag(id, "worn"))
                .collect(),
            _ => return Err(NounFail::NoAll),
        };
        if items.is_empty() { Err(NounFail::AllNone) } else { Ok(items) }
    }

    /// Picks the option the player meant when answering "which do you mean?".
    pub fn pick(&self, world: &World, options: &[EntityId], reply: &[String]) -> Option<EntityId> {
        let words: Vec<&String> = reply.iter().filter(|w| !self.articles.contains(*w)).collect();
        if words.is_empty() {
            return None;
        }
        let hits = self.match_scope(world, options, &words, true);
        (hits.len() == 1).then(|| hits[0])
    }

    /// Verb words worth suggesting here: verbs that something in scope
    /// handles, plus a few basics.
    pub fn suggestions(&self, world: &World) -> Vec<String> {
        let story = world.story();
        let first_word = |key: &str| -> Option<String> {
            let v = story.verbs.iter().find(|v| v.key == key)?;
            v.patterns.iter().find_map(|p| match p.0.first() {
                Some(PatternToken::Word(w)) if w.len() > 1 => Some(w.clone()),
                _ => None,
            })
        };
        let mut keys: Vec<String> = Vec::new();
        for id in world.scope() {
            for h in &world.def(id).handlers {
                if story.verbs.iter().any(|v| v.key == h.event) && !keys.contains(&h.event) {
                    keys.push(h.event.clone());
                }
            }
        }
        for basic in ["examine", "take", "look", "inventory"] {
            if !keys.iter().any(|k| k == basic) {
                keys.push(basic.to_string());
            }
        }
        keys.iter().filter_map(|k| first_word(k)).take(6).map(|w| w.to_uppercase()).collect()
    }

    /// Words that finish `partial`, for Tab in the player: at the start of a
    /// command its first words (the game's own verbs only once something
    /// here handles them, so a magic word isn't given away), then the names
    /// of what the player can see or carry, places they've been, directions
    /// and the little words in commands ("with", "to"). Nothing hidden.
    pub fn completions(&self, world: &World, first: bool, partial: &str) -> Vec<String> {
        let partial = partial.to_lowercase();
        if partial.is_empty() {
            return vec![];
        }
        let story = world.story();
        let handled: Vec<&str> = world.scope().into_iter().flat_map(|id| world.def(id).handlers.iter().map(|h| h.event.as_str())).collect();
        let verbs = story.verbs.iter().filter(|v| v.origin == VerbOrigin::Stdlib || handled.iter().any(|h| h.trim_start_matches("before_").trim_start_matches("after_") == v.key));
        let mut words: Vec<String> = Vec::new();
        let add = |words: &mut Vec<String>, w: &str| {
            if w.len() > partial.len() && w.starts_with(&partial) && w.chars().all(|c| c.is_alphabetic() || c == '-' || c == '\'') && !words.iter().any(|x| x == w) {
                words.push(w.to_string());
            }
        };
        if first {
            for v in verbs {
                for p in &v.patterns {
                    if let Some(PatternToken::Word(w)) = p.0.first() {
                        add(&mut words, w);
                    }
                }
            }
            for d in self.directions.keys() {
                add(&mut words, d);
            }
            words.sort();
            return words;
        }
        // Names first: what's here and carried, then places been.
        for id in world.scope().into_iter().filter(|&id| id != PLAYER) {
            let v = &world.def(id).vocab;
            for w in v.adjectives.iter().chain(&v.nouns) {
                add(&mut words, w);
            }
        }
        for &r in &world.state.visited {
            for w in world.def(r).name.to_lowercase().split_whitespace() {
                add(&mut words, w);
            }
        }
        let names = words.len();
        for d in self.directions.keys() {
            add(&mut words, d);
        }
        // The little words in commands (WITH, INTO), not the rest of a
        // pattern, which could name something the player hasn't seen.
        let stop = &story.lexicon.stopwords;
        for v in verbs {
            for p in &v.patterns {
                for t in p.0.iter().skip(1) {
                    if let PatternToken::Word(w) = t
                        && stop.contains(w)
                    {
                        add(&mut words, w);
                    }
                }
            }
        }
        words[..names].sort();
        words[names..].sort();
        words
    }

    /// The display word for a direction: its own name if the lexicon has
    /// it ("up", not "upstairs"), otherwise the longest lexicon word for it.
    pub fn direction_word(&self, d: Direction) -> String {
        if self.directions.get(d.key()) == Some(&d) {
            return d.key().to_string();
        }
        self.directions
            .iter()
            .filter(|(_, v)| **v == d)
            .map(|(k, _)| k)
            .max_by_key(|k| (k.len(), std::cmp::Reverse((*k).clone())))
            .cloned()
            .unwrap_or_else(|| d.key().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typo_distance_counts_swaps_as_one() {
        assert_eq!(distance("tkae", "take"), 1);
        assert_eq!(distance("serch", "search"), 1);
        assert_eq!(distance("north", "north"), 0);
    }

    #[test]
    fn near_misses_include_ties() {
        let words: Vec<String> = ["take", "talk", "search", "x"].iter().map(|s| s.to_string()).collect();
        assert_eq!(near_misses("tak", &words), vec!["take".to_string(), "talk".to_string()]);
        assert_eq!(near_misses("sarch", &words), vec!["search".to_string()]);
        assert!(near_misses("xyzzy", &words).is_empty());
        assert!(near_misses("ta", &words).is_empty());
    }

    #[test]
    fn near_misses_allow_two_slips_only_in_long_words() {
        let words: Vec<String> = ["order", "shove", "examine"].iter().map(|s| s.to_string()).collect();
        assert!(near_misses("ponder", &words).is_empty());
        assert!(near_misses("shiver", &words).is_empty());
        assert_eq!(near_misses("exmaine", &words), vec!["examine".to_string()]);
        assert_eq!(near_misses("exzmnie", &words), vec!["examine".to_string()]);
    }
}
