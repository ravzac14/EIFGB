//! Parses text templates into IR segments.
//!
//! Syntax:
//! * `${expr}` inserts the value of a Lua expression;
//! * `${if cond}…${elseif cond}…${else}…${end}` is a conditional.
//!
//! When scanning for the closing `}`, the parser skips Lua string
//! literals and balanced braces, so `${ {1,2} }` and `${"}"}` work.

use eifgb_ir::{Code, Segment, Text};

struct Frame {
    cond: Code,
    then: Vec<Segment>,
    otherwise: Vec<Segment>,
    in_else: bool,
    /// The frame was opened by `elseif`, so it closes along with its parent.
    chained: bool,
}

impl Frame {
    fn into_segment(self) -> Segment {
        Segment::If {
            cond: self.cond,
            then: Text(self.then),
            otherwise: Text(self.otherwise),
        }
    }
}

pub fn parse(src: &str, origin: &str) -> Result<Text, String> {
    let mut root: Vec<Segment> = Vec::new();
    let mut stack: Vec<Frame> = Vec::new();
    let code = |s: &str| Code { src: s.to_string(), origin: origin.to_string() };

    fn current<'a>(root: &'a mut Vec<Segment>, stack: &'a mut [Frame]) -> &'a mut Vec<Segment> {
        match stack.last_mut() {
            None => root,
            Some(f) if f.in_else => &mut f.otherwise,
            Some(f) => &mut f.then,
        }
    }

    let mut rest = src;
    while let Some(start) = rest.find("${") {
        if start > 0 {
            current(&mut root, &mut stack).push(Segment::Lit(rest[..start].to_string()));
        }
        let after = &rest[start + 2..];
        let close = find_close(after).ok_or_else(|| {
            format!("unclosed '${{' starting at \"${{{}\"", preview(after))
        })?;
        let inner = after[..close].trim();
        rest = &after[close + 1..];

        if inner.is_empty() {
            return Err("empty '${}'".into());
        }
        if inner == "else" {
            match stack.last_mut() {
                Some(f) if !f.in_else => f.in_else = true,
                Some(_) => return Err("'${else}' appears twice in the same '${if}'".into()),
                None => return Err("'${else}' without a matching '${if ...}'".into()),
            }
        } else if inner == "end" {
            let Some(frame) = stack.pop() else {
                return Err("'${end}' without a matching '${if ...}'".into());
            };
            let mut chained = frame.chained;
            let mut seg = frame.into_segment();
            while chained {
                let mut parent = stack.pop().expect("chained frame always has a parent");
                parent.otherwise.push(seg);
                chained = parent.chained;
                seg = parent.into_segment();
            }
            current(&mut root, &mut stack).push(seg);
        } else if let Some(cond) = keyword_arg(inner, "elseif") {
            match stack.last_mut() {
                Some(f) if !f.in_else => f.in_else = true,
                Some(_) => return Err("'${elseif}' after '${else}'".into()),
                None => return Err("'${elseif}' without a matching '${if ...}'".into()),
            }
            stack.push(Frame {
                cond: code(cond?),
                then: Vec::new(),
                otherwise: Vec::new(),
                in_else: false,
                chained: true,
            });
        } else if let Some(cond) = keyword_arg(inner, "if") {
            stack.push(Frame {
                cond: code(cond?),
                then: Vec::new(),
                otherwise: Vec::new(),
                in_else: false,
                chained: false,
            });
        } else {
            current(&mut root, &mut stack).push(Segment::Expr(code(inner)));
        }
    }
    if !rest.is_empty() {
        current(&mut root, &mut stack).push(Segment::Lit(rest.to_string()));
    }
    if let Some(f) = stack.last() {
        return Err(format!("'${{if {}}}' is missing its '${{end}}'", f.cond.src));
    }
    Ok(Text(root))
}

/// `if cond` → `Some(Ok("cond"))`; bare `if` → `Some(Err)`; anything else → `None`.
fn keyword_arg<'a>(inner: &'a str, kw: &str) -> Option<Result<&'a str, String>> {
    let rest = inner.strip_prefix(kw)?;
    if rest.is_empty() {
        return Some(Err(format!("'${{{kw}}}' needs a condition")));
    }
    // `iffy` is an identifier, not `if fy`.
    let next = rest.chars().next().unwrap();
    if next.is_alphanumeric() || next == '_' {
        return None;
    }
    let cond = rest.trim();
    if cond.is_empty() {
        return Some(Err(format!("'${{{kw}}}' needs a condition")));
    }
    Some(Ok(cond))
}

/// Byte offset of the `}` that closes a `${`, or `None` if there isn't one.
fn find_close(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let mut depth = 0usize;
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            q @ (b'"' | b'\'') => {
                i += 1;
                while i < b.len() && b[i] != q {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            b'[' => {
                // Lua long string: [[ ... ]] or [==[ ... ]==]
                let mut j = i + 1;
                while j < b.len() && b[j] == b'=' {
                    j += 1;
                }
                if j < b.len() && b[j] == b'[' {
                    let level = j - i - 1;
                    let closer = format!("]{}]", "=".repeat(level));
                    let end = s[j + 1..].find(&closer)?;
                    i = j + 1 + end + closer.len() - 1;
                }
            }
            b'{' => depth += 1,
            b'}' if depth == 0 => return Some(i),
            b'}' => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    None
}

fn preview(s: &str) -> String {
    let p: String = s.chars().take(20).collect();
    if s.chars().count() > 20 { format!("{p}…") } else { p }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit(s: &str) -> Segment {
        Segment::Lit(s.into())
    }

    fn src_of(seg: &Segment) -> &str {
        match seg {
            Segment::Expr(c) => &c.src,
            Segment::If { cond, .. } => &cond.src,
            Segment::Lit(s) => s,
        }
    }

    #[test]
    fn plain_text_is_one_literal() {
        assert_eq!(parse("Hello.", "t").unwrap().0, vec![lit("Hello.")]);
    }

    #[test]
    fn expressions_and_braces() {
        let t = parse(r#"A ${x} B ${ {1,2} } C ${"}"}"#, "t").unwrap();
        assert_eq!(t.0.len(), 6);
        assert_eq!(src_of(&t.0[1]), "x");
        assert_eq!(src_of(&t.0[3]), "{1,2}");
        assert_eq!(src_of(&t.0[5]), r#""}""#);
    }

    #[test]
    fn if_else() {
        let t = parse("It is ${if self.open}open${else}shut${end}.", "t").unwrap();
        let Segment::If { cond, then, otherwise } = &t.0[1] else { panic!() };
        assert_eq!(cond.src, "self.open");
        assert_eq!(then.0, vec![lit("open")]);
        assert_eq!(otherwise.0, vec![lit("shut")]);
        assert_eq!(t.0[2], lit("."));
    }

    #[test]
    fn elseif_chain_nests_and_closes_with_one_end() {
        let t = parse("${if a}A${elseif b}B${else}C${end}!", "t").unwrap();
        assert_eq!(t.0.len(), 2);
        let Segment::If { otherwise, .. } = &t.0[0] else { panic!() };
        let Segment::If { cond, then, otherwise: inner_else } = &otherwise.0[0] else { panic!() };
        assert_eq!(cond.src, "b");
        assert_eq!(then.0, vec![lit("B")]);
        assert_eq!(inner_else.0, vec![lit("C")]);
    }

    #[test]
    fn nested_if() {
        let t = parse("${if a}${if b}AB${end}${end}", "t").unwrap();
        let Segment::If { then, .. } = &t.0[0] else { panic!() };
        assert!(matches!(then.0[0], Segment::If { .. }));
    }

    #[test]
    fn identifiers_starting_with_keywords_are_expressions() {
        let t = parse("${iffy} ${ending}", "t").unwrap();
        assert!(matches!(t.0[0], Segment::Expr(_)));
        assert!(matches!(t.0[2], Segment::Expr(_)));
    }

    #[test]
    fn errors() {
        assert!(parse("${x", "t").unwrap_err().contains("unclosed"));
        assert!(parse("${if a}x", "t").unwrap_err().contains("missing"));
        assert!(parse("x${end}", "t").unwrap_err().contains("without"));
        assert!(parse("${else}", "t").is_err());
        assert!(parse("${if a}${else}${else}${end}", "t").is_err());
        assert!(parse("${if}", "t").is_err());
        assert!(parse("${}", "t").is_err());
    }
}
