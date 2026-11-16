//! The versioned standard library.
//!
//! Each release adds a new `stdlib/v<N>/` folder and never edits an old
//! one. A game names its version in `game.stdlib`, and the compiler
//! copies that version into the story IR. Rebuilding an old game with a
//! newer compiler therefore keeps its parser behaviour unchanged.
//!
//! This crate holds the raw data. The compiler parses the patterns and
//! templates the same way it parses author source, so both get the same
//! validation.

use eifgb_ir::{BuiltinAction, Lexicon};
use indexmap::IndexMap;
use serde::Deserialize;

/// The newest standard library version. New games use it by default.
pub const LATEST: u32 = 1;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stdlib {
    pub verbs: IndexMap<String, StdVerb>,
    pub lexicon: Lexicon,
    /// Message key → template source.
    pub messages: IndexMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StdVerb {
    pub action: BuiltinAction,
    pub patterns: Vec<String>,
}

/// Raw source for a version and language, if that combination exists.
pub fn source(version: u32, language: &str) -> Option<&'static str> {
    match (version, language) {
        (1, "en") => Some(include_str!("../stdlib/v1/en.yaml")),
        _ => None,
    }
}

/// Every language available for a version.
pub fn languages(version: u32) -> &'static [&'static str] {
    match version {
        1 => &["en"],
        _ => &[],
    }
}

#[derive(Debug)]
pub enum LoadError {
    Unknown { version: u32, language: String },
    Malformed(serde_yaml::Error),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Unknown { version, language } => {
                write!(f, "no standard library v{version} for language '{language}'")
            }
            LoadError::Malformed(e) => write!(f, "standard library data is malformed: {e}"),
        }
    }
}

impl std::error::Error for LoadError {}

pub fn load(version: u32, language: &str) -> Result<Stdlib, LoadError> {
    let src = source(version, language).ok_or_else(|| LoadError::Unknown {
        version,
        language: language.to_string(),
    })?;
    serde_yaml::from_str(src).map_err(LoadError::Malformed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_published_version_loads() {
        for v in 1..=LATEST {
            for lang in languages(v) {
                let lib = load(v, lang).unwrap_or_else(|e| panic!("v{v}/{lang}: {e}"));
                assert!(!lib.verbs.is_empty());
                assert!(!lib.messages.is_empty());
            }
        }
    }

    #[test]
    fn lexicon_is_lowercase() {
        let lib = load(LATEST, "en").unwrap();
        let lx = &lib.lexicon;
        let words = lx
            .phrases
            .iter()
            .flat_map(|(a, b)| [a, b])
            .chain(lx.directions.keys())
            .chain(&lx.articles)
            .chain(&lx.filler)
            .chain(&lx.pronouns)
            .chain(&lx.all_words);
        for w in words {
            assert_eq!(w, &w.to_lowercase(), "lexicon entry '{w}' must be lowercase");
        }
    }
}
