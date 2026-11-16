//! Language-specific rules the engine needs in Rust: word stemming,
//! articles and joining lists. Word lists live in the stdlib lexicon,
//! and message text lives in the message catalog. Adding a language
//! means adding a module here, alongside its lexicon and catalog.

pub struct Lang {
    /// Possible base forms of an inflected word ("grabbed" → "grab", "boxes" → "box").
    pub stems: fn(&str) -> Vec<String>,
    /// Indefinite form: "a brass key", "an oil can". `bare` means no article.
    pub a: fn(name: &str, bare: bool) -> String,
    /// Definite form: "the brass key".
    pub the: fn(name: &str, bare: bool) -> String,
    /// "x, y and z"
    pub list_and: fn(&[String]) -> String,
    /// "x, y or z"
    pub list_or: fn(&[String]) -> String,
}

pub fn for_language(code: &str) -> Option<&'static Lang> {
    match code {
        "en" => Some(&en::LANG),
        _ => None,
    }
}

mod en {
    use super::Lang;

    pub static LANG: Lang = Lang { stems, a, the, list_and, list_or };

    fn stems(w: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut push = |s: String| {
            if s.len() >= 2 && !out.contains(&s) {
                out.push(s);
            }
        };
        let undouble = |s: &str| -> Option<String> {
            let b = s.as_bytes();
            (b.len() >= 2 && b[b.len() - 1] == b[b.len() - 2]).then(|| s[..s.len() - 1].to_string())
        };
        for suffix in ["ing", "ed"] {
            if let Some(s) = w.strip_suffix(suffix) {
                push(s.to_string());
                push(format!("{s}e"));
                if let Some(u) = undouble(s) {
                    push(u);
                }
            }
        }
        if let Some(s) = w.strip_suffix("ied").or_else(|| w.strip_suffix("ies")) {
            push(format!("{s}y"));
        }
        if let Some(s) = w.strip_suffix("es") {
            push(s.to_string());
        }
        if let Some(s) = w.strip_suffix('s')
            && !s.ends_with('s')
        {
            push(s.to_string());
        }
        out
    }

    fn a(name: &str, bare: bool) -> String {
        if bare {
            return name.to_string();
        }
        let vowel = name.chars().next().is_some_and(|c| "aeiouAEIOU".contains(c));
        format!("{} {name}", if vowel { "an" } else { "a" })
    }

    fn the(name: &str, bare: bool) -> String {
        if bare { name.to_string() } else { format!("the {name}") }
    }

    fn join(items: &[String], word: &str) -> String {
        match items {
            [] => String::new(),
            [one] => one.clone(),
            [rest @ .., last] => format!("{} {word} {last}", rest.join(", ")),
        }
    }

    fn list_and(items: &[String]) -> String {
        join(items, "and")
    }

    fn list_or(items: &[String]) -> String {
        join(items, "or")
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn stemming() {
            assert!(stems("grabbed").contains(&"grab".to_string()));
            assert!(stems("taking").contains(&"take".to_string()));
            assert!(stems("keys").contains(&"key".to_string()));
            assert!(stems("boxes").contains(&"box".to_string()));
            assert!(stems("batteries").contains(&"battery".to_string()));
            assert!(!stems("glass").contains(&"glas".to_string()));
        }

        #[test]
        fn articles_and_lists() {
            assert_eq!(a("oil can", false), "an oil can");
            assert_eq!(a("key", false), "a key");
            assert_eq!(a("yourself", true), "yourself");
            assert_eq!(list_and(&["a".into(), "b".into(), "c".into()]), "a, b and c");
            assert_eq!(list_or(&["a".into(), "b".into()]), "a or b");
            assert_eq!(list_and(&["a".into()]), "a");
        }
    }
}
