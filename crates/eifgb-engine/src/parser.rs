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
use eifgb_ir::{BuiltinAction, Direction, EntityId, PatternToken, Slot, Story, Trait, VerbAction, PLAYER};
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
    NeedNoun { prefix: Vec<String>, verb: String, target: Option<EntityId>, word: Option<String> },
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
        }
    }
}

fn words(s: &str) -> Vec<String> {
    s.split_whitespace().map(str::to_string).collect()
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

/// The candidate closest to `word`, if it's unambiguous and close enough to be a typo.
fn closest<'a>(word: &str, candidates: impl IntoIterator<Item = &'a String>) -> Option<&'a String> {
    if word.chars().count() < 3 || word.starts_with(ENTITY_TOKEN) {
        return None;
    }
    let limit = if word.chars().count() <= 4 { 1 } else { 2 };
    let mut best: Option<(usize, &String)> = None;
    let mut tie = false;
    for c in candidates {
        let d = distance(word, c);
        if d == 0 || d > limit {
            continue;
        }
        match best {
            Some((bd, _)) if d > bd => {}
            Some((bd, _)) if d == bd => tie = true,
            _ => {
                best = Some((d, c));
                tie = false;
            }
        }
    }
    if tie { None } else { best.map(|(_, c)| c) }
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
        }
    }

    pub fn lang(&self) -> &'static Lang {
        self.lang
    }

    /// Splits raw input into separate commands, each one a list of
    /// lowercase words. "." and ";" separate commands, and so does "then".
    pub fn split_commands(&self, input: &str) -> Vec<Vec<String>> {
        let cleaned: String = input
            .to_lowercase()
            .chars()
            .map(|c| match c {
                '.' | ';' | '!' | '?' => '.',
                c if c.is_alphanumeric() || c == '\'' || c == '-' || c == ENTITY_TOKEN => c,
                _ => ' ',
            })
            .collect();
        let mut out = Vec::new();
        for part in cleaned.split('.') {
            let mut cur = Vec::new();
            for w in part.split_whitespace() {
                if w == "then" {
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

    fn is_known_start(&self, w: &str) -> bool {
        self.verb_words.contains(w) || self.directions.contains_key(w) || self.phrases.iter().any(|(k, _)| k[0] == w)
    }

    /// Normalization: drop leading filler, give inflected verbs their base
    /// form, and rewrite synonym phrases at the start of the command.
    pub fn normalize(&self, mut toks: Vec<String>) -> Vec<String> {
        while let Some(f) = self.filler.iter().find(|f| toks.len() > f.len() && toks.starts_with(f)) {
            toks.drain(..f.len());
        }
        if let Some(first) = toks.first()
            && !self.is_known_start(first)
            && let Some(stem) = (self.lang.stems)(first).into_iter().find(|s| self.is_known_start(s))
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
        if toks.is_empty() {
            return Parsed::Fail(Failure::Empty);
        }
        let story = world.story();
        let mut toks = toks.to_vec();
        let mut cands = self.candidates(story, &toks);

        // Step 5: correct a misspelled verb or direction.
        if cands.is_empty() && !self.is_known_start(&toks[0]) {
            let starts: Vec<String> = self
                .verb_words
                .iter()
                .chain(self.directions.keys())
                .chain(self.phrases.iter().map(|(k, _)| &k[0]))
                .cloned()
                .collect();
            if let Some(fixed) = closest(&toks[0], &starts) {
                let mut fixed_toks = toks.clone();
                fixed_toks[0] = fixed.clone();
                let fixed_toks = self.normalize(fixed_toks);
                let fixed_cands = self.candidates(story, &fixed_toks);
                if !fixed_cands.is_empty() || fixed_toks.len() == 1 {
                    toks = fixed_toks;
                    cands = fixed_cands;
                }
            }
        }

        if cands.is_empty() {
            return self.no_pattern(world, &toks);
        }

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
        let lead: Vec<&str> = verb
            .patterns
            .first()
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
                    return Parsed::NeedNoun { prefix: toks.to_vec(), verb: Self::verb_name(verb), target: None, word: None };
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
                    verb: Self::verb_name(verb),
                    target: None,
                    word: None,
                },
                NounFail::NoPronoun => Parsed::Fail(Failure::NoPronoun),
                NounFail::NotFound { unknown_word: Some(w) } => Parsed::Fail(Failure::UnknownWord { word: w }),
                NounFail::NotFound { unknown_word: None } => Parsed::Fail(Failure::NotHere),
                NounFail::Scenery => Parsed::Fail(Failure::Scenery),
                NounFail::Structural => Parsed::Fail(Failure::Partial { verb: Self::verb_name(verb) }),
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
                let ids = self.resolve(world, &toks[span.0..span.1], None, None).map_err(|e| fail(e, span))?;
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
        // What merchants here are selling can be looked at and bought.
        if matches!(action, Some(BuiltinAction::Buy | BuiltinAction::Examine | BuiltinAction::Offer)) {
            for m in world.merchants_here() {
                scope.extend(world.contents(m).into_iter().filter(|&i| world.def(i).price.is_some()));
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

        let mut matches = self.match_scope(world, &scope, &words);
        if matches.is_empty() {
            // Step 4: words the player read in a description: "x glint" after "Something
            // glints at the bottom." Real names always win; this is only a fallback.
            matches = self.match_descriptions(world, &scope, &words);
            if matches.is_empty() && words.iter().all(|w| self.describes(world.here(), w)) {
                return Err(NounFail::Scenery);
            }
        }
        if matches.is_empty() {
            // Step 5: fix misspelled nouns, using only words of things in scope.
            let scope_vocab: Vec<String> = scope
                .iter()
                .flat_map(|e| {
                    let v = &world.def(*e).vocab;
                    v.nouns.iter().chain(&v.adjectives).cloned().collect::<Vec<_>>()
                })
                .collect();
            let fixed: Option<Vec<String>> = words
                .iter()
                .map(|w| {
                    if scope_vocab.contains(w) || self.stem_in(w, &scope_vocab) {
                        Some((*w).clone())
                    } else {
                        closest(w, &scope_vocab).cloned()
                    }
                })
                .collect();
            if let Some(fixed) = fixed {
                let refs: Vec<&String> = fixed.iter().collect();
                matches = self.match_scope(world, &scope, &refs);
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
                    .find(|w| !self.vocab_words.contains(**w) && !self.stem_known(w) && !named(w))
                    .map(|w| (*w).clone());
                Err(NounFail::NotFound { unknown_word: unknown })
            }
            1 => Ok(matches),
            _ => {
                let narrowed = self.prefer(world, &matches, action);
                if narrowed.len() == 1 { Ok(narrowed) } else { Err(NounFail::Ambiguous(narrowed)) }
            }
        }
    }

    fn stem_in(&self, w: &str, vocab: &[String]) -> bool {
        (self.lang.stems)(w).iter().any(|s| vocab.contains(s))
    }

    fn stem_known(&self, w: &str) -> bool {
        self.corpse_words.contains(w)
            || (self.lang.stems)(w).iter().any(|s| self.vocab_words.contains(s) || self.corpse_words.contains(s))
            || self.described_words.contains(w)
            || (self.lang.stems)(w).iter().any(|s| self.described_words.contains(s))
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
    fn match_scope(&self, world: &World, scope: &[EntityId], words: &[&String]) -> Vec<EntityId> {
        let mut best_score = 0;
        let mut out = Vec::new();
        for &id in scope {
            let v = &world.def(id).vocab;
            let dead = world.is_dead(id);
            // "corpse" for the dead; a name the player gave it, as a noun.
            let corpse_word = |s: &str| (dead && self.corpse_words.contains(s)) || world.named_word(id, s);
            let known = |w: &str| {
                let check = |s: &str| v.nouns.iter().any(|n| n == s) || v.adjectives.iter().any(|a| a == s) || corpse_word(s);
                check(w) || (self.lang.stems)(w).iter().any(|s| check(s))
            };
            if v.is_empty() || !words.iter().all(|w| known(w)) {
                continue;
            }
            let last = words[words.len() - 1];
            let noun = |s: &str| v.nouns.iter().any(|n| n == s) || corpse_word(s);
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
        if filtered.is_empty() { matches.to_vec() } else { filtered }
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
        let hits = self.match_scope(world, options, &words);
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
    fn closest_refuses_ties_and_short_words() {
        let words: Vec<String> = ["take", "talk", "search"].iter().map(|s| s.to_string()).collect();
        assert_eq!(closest("serch", &words).map(String::as_str), Some("search"));
        assert_eq!(closest("tak", &words), None, "take and talk tie");
        assert_eq!(closest("xx", &words), None);
    }
}
