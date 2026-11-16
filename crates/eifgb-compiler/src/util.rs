//! Small helpers: identifier checks, "did you mean" suggestions and
//! scanning Lua for calls with string-literal arguments.

pub const LUA_KEYWORDS: &[&str] = &[
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if",
    "in", "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
];

/// Names the script environment already uses, so no entity may take them.
pub const RESERVED: &[&str] = &["player", "world", "vars", "game", "args", "here", "self"];

/// IDs are lowercase snake_case, so they can be used as Lua fields (`world.<id>`).
pub fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase() || c == '_')
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

pub fn check_id(s: &str) -> Result<(), String> {
    if !is_identifier(s) {
        return Err(format!("'{s}' is not a valid ID; use lowercase letters, digits and '_'"));
    }
    if LUA_KEYWORDS.contains(&s) {
        return Err(format!("'{s}' is a Lua keyword and can't be used as an ID"));
    }
    Ok(())
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// The closest candidate, if it's close enough to be a likely typo.
pub fn suggest<'a>(word: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let limit = (word.chars().count() / 3).max(2);
    candidates
        .into_iter()
        .map(|c| (levenshtein(word, c), c))
        .filter(|(d, _)| *d <= limit)
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| c)
}

/// " (did you mean 'x'?)", or "" when there's no good suggestion.
pub fn did_you_mean<'a>(word: &str, candidates: impl IntoIterator<Item = &'a str>) -> String {
    suggest(word, candidates)
        .map(|s| format!(" (did you mean '{s}'?)"))
        .unwrap_or_default()
}

/// Finds calls like `name("literal")` and `name('literal')` in Lua source.
/// This is a light scan, not a Lua parser. It is enough to check
/// references such as `end_game("saved")` at compile time.
pub fn literal_calls<'a>(src: &'a str, name: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(pos) = src[from..].find(name) {
        let start = from + pos;
        from = start + name.len();
        let before_ok = src[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '.' || c == ':'));
        if !before_ok {
            continue;
        }
        let rest = src[from..].trim_start();
        let Some(rest) = rest.strip_prefix('(') else { continue };
        let rest = rest.trim_start();
        let Some(q) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') else { continue };
        let body = &rest[1..];
        if let Some(end) = body.find(q) {
            out.push(&body[..end]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers() {
        assert!(check_id("brass_key").is_ok());
        assert!(check_id("room2").is_ok());
        assert!(check_id("Brass").is_err());
        assert!(check_id("2room").is_err());
        assert!(check_id("end").is_err());
        assert!(check_id("brass-key").is_err());
    }

    #[test]
    fn suggestions() {
        assert_eq!(suggest("tide_pol", ["shore", "tide_pool"]), Some("tide_pool"));
        assert_eq!(suggest("xyzzy", ["shore", "tide_pool"]), None);
        assert_eq!(suggest("serch", ["search", "take"]), Some("search"));
    }

    #[test]
    fn scans_literal_calls() {
        let src = r#"if x then end_game("saved") else my_end_game("no") end; end_game 'x'; end_game( 'lost' )"#;
        assert_eq!(literal_calls(src, "end_game"), vec!["saved", "lost"]);
    }
}
