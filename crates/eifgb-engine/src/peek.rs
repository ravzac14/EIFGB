//! PEEK: a quick look into the next room, through a door or an open way,
//! without going in. The player gets an impression, never the room's
//! description: whether anyone's there (and asleep), bodies, how much is
//! lying about, and whether a light is burning. Nobody notices.

use crate::game::{Game, LineKind};
use crate::script::{Ctx, ScriptHost};
use eifgb_ir::{Direction, EntityId, EntityKind, Exit, Trait, PLAYER};

impl<H: ScriptHost> Game<H> {
    pub(crate) fn peek_action(&mut self, through: Option<EntityId>, dir: Option<Direction>, ctx: &Ctx) -> bool {
        let here = self.world.here();
        let exits: Vec<Exit> = self.world.def(here).exits.clone();
        let exit = match (through, dir) {
            (Some(door), _) => {
                if !self.world.has(door, Trait::Door) {
                    self.msg("peek.not_door", ctx);
                    return false;
                }
                exits.into_iter().find(|e| e.through == Some(door) && e.to.is_some())
            }
            (None, Some(d)) => {
                exits.into_iter().find(|e| e.direction == d && e.to.is_some() && self.world.exit_visible(e))
            }
            (None, None) => None,
        };
        let Some(exit) = exit else {
            self.no_turn = true;
            self.msg("peek.no_way", ctx);
            return false;
        };
        let room = exit.to.unwrap_or(here);
        let place = self.world.state.visited.contains(&room).then(|| self.world.def(room).name.clone());
        let mut c = Ctx::new();
        if let Some(p) = &place {
            c = c.text("place", p.clone());
        }
        let opening = match exit.through {
            Some(door) => self.msg_str("peek.through", &c.clone().entity("door", door)),
            None => {
                let d = self.grammar.direction_word(exit.direction);
                self.msg_str("peek.direction", &c.clone().text("direction", d))
            }
        };
        let mut sentences = vec![opening];

        let in_room = |w: &crate::World, id: EntityId| w.location(id) == Some(room);
        let lit = (0..self.world.story().entities.len() as u32)
            .map(EntityId)
            .any(|i| self.world.room_of(i) == Some(room) && self.world.is_burning(i) && self.world.in_plain_sight(i));
        if !self.world.is_lit(room) && !self.world.player_sees_in_dark() {
            sentences.push(self.msg_str("peek.dark", &Ctx::new()));
        } else {
            let people: Vec<EntityId> = self
                .world
                .contents(room)
                .into_iter()
                .filter(|&c| c != PLAYER && self.world.is_active(c) && !self.world.is_hidden(c))
                .collect();
            let asleep = people.iter().filter(|&&p| self.world.is_asleep(p)).count();
            let bodies = self.world.contents(room).into_iter().filter(|&c| self.world.is_dead(c)).count();
            let things = self
                .world
                .contents(room)
                .into_iter()
                .filter(|&c| {
                    in_room(&self.world, c)
                        && self.world.def(c).kind == EntityKind::Object
                        && !self.world.has(c, Trait::Scenery)
                        && !self.world.is_hidden(c)
                })
                .count();
            if people.is_empty() && bodies == 0 && things == 0 {
                sentences.push(self.msg_str("peek.empty", &Ctx::new()));
            }
            let who = match people.len() {
                0 => None,
                1 => Some("peek.one"),
                2 => Some("peek.two"),
                _ => Some("peek.several"),
            };
            if let Some(k) = who {
                sentences.push(self.msg_str(k, &Ctx::new()));
                let sleep = match (asleep, people.len()) {
                    (0, _) => None,
                    (a, n) if a == n && n == 1 => Some("peek.asleep_one"),
                    (a, n) if a == n => Some("peek.asleep_all"),
                    (1, 2) => Some("peek.asleep_one_of_two"),
                    _ => Some("peek.asleep_some"),
                };
                if let Some(k) = sleep {
                    sentences.push(self.msg_str(k, &Ctx::new()));
                }
            }
            if bodies > 0 {
                sentences.push(self.msg_str("peek.bodies", &Ctx::new()));
            }
            let stuff = match things {
                0 => None,
                1..=2 => Some("peek.few"),
                3..=5 => Some("peek.some"),
                _ => Some("peek.lots"),
            };
            if let Some(k) = stuff {
                sentences.push(self.msg_str(k, &Ctx::new()));
            }
            if self.world.fire_in(room).is_some() {
                sentences.push(self.msg_str("peek.fire", &Ctx::new()));
            } else if lit {
                sentences.push(self.msg_str("peek.lit", &Ctx::new()));
            }
        }
        self.push(LineKind::Text, sentences.join(" "));
        true
    }
}
