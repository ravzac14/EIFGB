//! `kinds:`: something written once and used again. A kind is a set of
//! fields (`guard: { traits: [...], faction: watch, health: 5 }`), and a
//! room, object or character that's `like: guard` starts from it, with its
//! own fields on top. Maps (`state`, `on`…) merge, `traits` add up, and
//! anything else of its own replaces the kind's. A kind can be `like`
//! another. It's done to the YAML before anything reads it, so the rest
//! of the compiler never sees a kind.

use crate::diag::Diagnostics;
use crate::util::did_you_mean;
use serde_yaml::{Mapping, Value};

const SECTIONS: [&str; 3] = ["rooms", "objects", "characters"];

/// Expands every `like:` in the game, and takes out the `kinds:` section.
/// Returns whether there was anything to do.
pub fn expand(doc: &mut Value, d: &mut Diagnostics) -> bool {
    let Some(top) = doc.as_mapping_mut() else { return false };
    let kinds = top.remove("kinds");
    let uses_like = SECTIONS.iter().any(|s| {
        top.get(*s).and_then(Value::as_mapping).is_some_and(|m| m.values().any(|e| e.as_mapping().is_some_and(|e| e.contains_key("like"))))
    });
    if kinds.is_none() && !uses_like {
        return false;
    }
    let kinds = match kinds {
        None => Mapping::new(),
        Some(Value::Mapping(m)) => m,
        Some(Value::Null) => Mapping::new(),
        Some(_) => {
            d.error("kinds", "'kinds' is a map of kinds: `guard: { traits: [...], health: 5 }`");
            Mapping::new()
        }
    };
    let names: Vec<String> = kinds.keys().filter_map(|k| k.as_str().map(String::from)).collect();
    for section in SECTIONS {
        let Some(list) = top.get_mut(section).and_then(Value::as_mapping_mut) else { continue };
        for (id, e) in list.iter_mut() {
            let Some(e) = e.as_mapping_mut() else { continue };
            let Some(like) = e.remove("like") else { continue };
            let path = format!("{section}.{}.like", id.as_str().unwrap_or("?"));
            let mut base = Mapping::new();
            for k in like_list(&like, &path, d) {
                if let Some(m) = resolve(&kinds, &k, &names, &mut Vec::new(), &path, d) {
                    base = merge(base, m);
                }
            }
            *e = merge(base, std::mem::take(e));
        }
    }
    true
}

/// `like: guard` or `like: [guard, armed]`.
fn like_list(v: &Value, path: &str, d: &mut Diagnostics) -> Vec<String> {
    match v {
        Value::String(s) => vec![s.trim().to_string()],
        Value::Sequence(l) => l.iter().filter_map(|x| x.as_str().map(|s| s.trim().to_string())).collect(),
        _ => {
            d.error(path, "'like' names a kind, or a list of them");
            Vec::new()
        }
    }
}

/// A kind's fields, with those of the kinds it's like underneath.
fn resolve(kinds: &Mapping, name: &str, names: &[String], seen: &mut Vec<String>, path: &str, d: &mut Diagnostics) -> Option<Mapping> {
    if seen.iter().any(|s| s == name) {
        d.error(path, format!("the kind '{name}' is like itself, by way of {}", seen.join(", ")));
        return None;
    }
    let Some(k) = kinds.get(name) else {
        let hint = did_you_mean(name, names.iter().map(String::as_str));
        d.error(path, format!("no kind called '{name}'{hint}; add it under 'kinds:'"));
        return None;
    };
    let Some(k) = k.as_mapping() else {
        d.error(format!("kinds.{name}"), "a kind is a map of fields: `{ traits: [portable], size: 2 }`");
        return None;
    };
    let mut own = k.clone();
    let Some(like) = own.remove("like") else { return Some(own) };
    seen.push(name.to_string());
    let mut base = Mapping::new();
    for other in like_list(&like, &format!("kinds.{name}.like"), d) {
        if let Some(m) = resolve(kinds, &other, names, seen, &format!("kinds.{name}.like"), d) {
            base = merge(base, m);
        }
    }
    seen.pop();
    Some(merge(base, own))
}

/// `over` on top of `base`: maps merge, traits add up, the rest is replaced.
fn merge(mut base: Mapping, over: Mapping) -> Mapping {
    for (k, v) in over {
        let merged = match (base.remove(&k), v) {
            (Some(Value::Mapping(a)), Value::Mapping(b)) => Value::Mapping(merge(a, b)),
            (Some(Value::Sequence(mut a)), Value::Sequence(b)) if k.as_str() == Some("traits") => {
                for t in b {
                    if !a.contains(&t) {
                        a.push(t);
                    }
                }
                Value::Sequence(a)
            }
            (_, v) => v,
        };
        base.insert(k, merged);
    }
    base
}
