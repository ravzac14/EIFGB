//! Parses verb grammar patterns such as `"fill {target} with {tool}"`.

use eifgb_ir::{Pattern, PatternToken, Slot};

pub fn parse(src: &str) -> Result<Pattern, String> {
    let mut tokens = Vec::new();
    let mut seen: Vec<Slot> = Vec::new();
    for raw in src.split_whitespace() {
        if let Some(name) = raw.strip_prefix('{') {
            let name = name
                .strip_suffix('}')
                .ok_or_else(|| format!("'{raw}' is missing its closing '}}'"))?;
            let slot = Slot::from_key(name).ok_or_else(|| {
                format!("unknown slot '{{{name}}}'; use {{target}}, {{tool}}, {{text}} or {{direction}}")
            })?;
            if seen.contains(&slot) {
                return Err(format!("slot '{{{name}}}' appears more than once"));
            }
            // A direction is always a single, known word, so it can sit next
            // to another slot ("throw {target} {direction}").
            if let Some(PatternToken::Slot(prev)) = tokens.last()
                && *prev != Slot::Direction
                && slot != Slot::Direction
            {
                return Err("two slots in a row can't be told apart; put a word between them".into());
            }
            seen.push(slot);
            tokens.push(PatternToken::Slot(slot));
        } else {
            if raw.contains(['{', '}']) {
                return Err(format!("'{raw}': slots must be separate words, like '{{target}}'"));
            }
            if !raw.chars().all(|c| c.is_alphanumeric() || c == '\'' || c == '-') {
                return Err(format!("'{raw}' contains punctuation; patterns are plain words"));
            }
            tokens.push(PatternToken::Word(raw.to_lowercase()));
        }
    }
    match tokens.as_slice() {
        [] => return Err("pattern is empty".into()),
        [PatternToken::Slot(Slot::Direction)] => {}
        [PatternToken::Slot(_), ..] => {
            return Err("a pattern must start with a word (only '{direction}' may stand alone)".into());
        }
        _ => {}
    }
    if seen.contains(&Slot::Tool) && !seen.contains(&Slot::Target) {
        return Err("'{tool}' needs a '{target}' in the same pattern".into());
    }
    Ok(Pattern(tokens))
}

pub fn display(p: &Pattern) -> String {
    p.0.iter()
        .map(|t| match t {
            PatternToken::Word(w) => w.clone(),
            PatternToken::Slot(s) => format!("{{{}}}", s.key()),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_words_and_slots() {
        let p = parse("Fill {target} with {tool}").unwrap();
        assert_eq!(display(&p), "fill {target} with {tool}");
        assert!(parse("{direction}").is_ok());
        assert!(parse("tune {target} to {text}").is_ok());
    }

    #[test]
    fn rejects_bad_patterns() {
        assert!(parse("").is_err());
        assert!(parse("{target} please").is_err());
        assert!(parse("give {target} {tool}").is_err());
        assert!(parse("use {tool}").is_err());
        assert!(parse("hit {thing}").is_err());
        assert!(parse("hit {target} {target}").is_err());
        assert!(parse("hit{target}").is_err());
        assert!(parse("hit {target}!").is_err());
    }
}
