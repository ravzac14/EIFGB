//! Rendering text templates, and reflowing prose for display.

use crate::script::{Ctx, Effect, ScriptError, ScriptHost};
use crate::world::World;
use eifgb_ir::{Segment, Text, VaryMode};

#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    pub text: String,
    /// Effects from any scripts the template ran. These are rare, e.g. a
    /// template that calls a helper function which uses `say`.
    pub effects: Vec<Effect>,
}

pub fn render(
    host: &mut dyn ScriptHost,
    world: &mut World,
    text: &Text,
    ctx: &Ctx,
) -> Result<Rendered, ScriptError> {
    let mut out = Rendered { text: String::new(), effects: Vec::new() };
    render_into(host, world, text, ctx, &mut out)?;
    Ok(out)
}

fn render_into(
    host: &mut dyn ScriptHost,
    world: &mut World,
    text: &Text,
    ctx: &Ctx,
    out: &mut Rendered,
) -> Result<(), ScriptError> {
    for seg in &text.0 {
        match seg {
            Segment::Lit(s) => out.text.push_str(s),
            Segment::Expr(code) => {
                let r = host.call(world, code, ctx)?;
                out.text.push_str(&r.value.to_text(world));
                out.effects.extend(r.effects);
            }
            Segment::If { cond, then, otherwise } => {
                let r = host.call(world, cond, ctx)?;
                out.effects.extend(r.effects);
                let branch = if r.value.truthy() { then } else { otherwise };
                render_into(host, world, branch, ctx, out)?;
            }
            Segment::Vary { key, mode, options } => {
                if options.is_empty() {
                    continue;
                }
                let n = options.len();
                // How many times it's been shown (for a random one, the last pick + 1).
                let seen = world.state.vary.get(key).copied().unwrap_or(0) as usize;
                let (i, next) = match mode {
                    VaryMode::Stop => (seen.min(n - 1), (seen + 1).min(n)),
                    VaryMode::Cycle => (seen % n, (seen + 1) % n),
                    VaryMode::Random => {
                        let mut i = world.state.rng.range(0, n as i64 - 1) as usize;
                        // Never the same twice in a row.
                        if n > 1 && seen == i + 1 {
                            i = (i + 1) % n;
                        }
                        (i, i + 1)
                    }
                };
                world.state.vary.insert(key.clone(), next as u32);
                render_into(host, world, &options[i], ctx, out)?;
            }
        }
    }
    Ok(())
}

/// Reflows source prose: single newlines are soft wraps and become
/// spaces, blank lines separate paragraphs, and runs of spaces collapse
/// to one. Paragraphs are joined with "\n\n".
pub fn reflow(s: &str) -> String {
    let mut paragraphs: Vec<String> = Vec::new();
    let mut cur = String::new();
    for line in s.lines() {
        let line = line.trim();
        if line.is_empty() {
            if !cur.is_empty() {
                paragraphs.push(std::mem::take(&mut cur));
            }
            continue;
        }
        for word in line.split_whitespace() {
            if !cur.is_empty() {
                cur.push(' ');
            }
            cur.push_str(word);
        }
    }
    if !cur.is_empty() {
        paragraphs.push(cur);
    }
    paragraphs.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::reflow;

    #[test]
    fn reflow_joins_soft_wraps_and_keeps_paragraphs() {
        let src = "Waves break against\n  black rocks.\n\n\nA path   climbs north.\n";
        assert_eq!(reflow(src), "Waves break against black rocks.\n\nA path climbs north.");
        assert_eq!(reflow(""), "");
    }
}
