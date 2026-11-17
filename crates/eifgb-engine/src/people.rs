//! Characters in the turn loop: their turns, fighting, and trading.
//!
//! [`crate::npc`] decides what characters do and changes the world. This
//! module runs their turns, describes what the player can see, and
//! carries out ATTACK, BUY, SELL, WARES and HEALTH.

use crate::game::{Game, LineKind};
use crate::npc::NpcEvent;
use crate::script::{Ctx, ScriptHost, ScriptValue};
use crate::world::{Fit, World};
use eifgb_ir::{Behavior, Code, EntityId, EntityKind, Phase, Value, PLAYER};

impl<H: ScriptHost> Game<H> {
    /// Every active character takes a turn, in the order they're declared.
    pub(crate) fn npc_turns(&mut self) {
        let npcs: Vec<EntityId> = (0..self.world.story().entities.len() as u32)
            .map(EntityId)
            .filter(|&id| self.world.def(id).npc.is_some())
            .collect();
        for npc in npcs {
            if self.end_requested.is_some() {
                return;
            }
            if !self.world.is_active(npc) {
                continue;
            }
            // A body of its own, left in plain sight: raising the alarm is
            // its whole turn, whatever its behaviour or script.
            if let Some(event) = self.world.discover_bodies(npc) {
                self.narrate(&event);
                self.check_defeats();
                continue;
            }
            match self.world.def(npc).npc.as_ref().and_then(|n| n.act.clone()) {
                Some(code) => {
                    self.run(&code, &Ctx::new().this(npc));
                }
                None => {
                    for event in self.world.npc_behave(npc) {
                        self.narrate(&event);
                    }
                }
            }
            self.check_defeats();
        }
    }

    /// Describes what a character did, if the player is there to see it.
    pub(crate) fn narrate(&mut self, event: &NpcEvent) {
        let here = self.world.here();
        match *event {
            NpcEvent::Moved { npc, from, to, direction } => {
                if from == here {
                    let dir = self.grammar.direction_word(direction);
                    self.msg("npc.leaves", &Ctx::new().entity("npc", npc).text("direction", dir));
                } else if to == here {
                    self.msg("npc.arrives", &Ctx::new().entity("npc", npc));
                }
            }
            NpcEvent::Attacked { attacker, target, weapon, tolerated, ref attitude, .. } => {
                let mut ctx = Ctx::new().entity("npc", attacker).entity("target", target);
                if let Some(w) = weapon {
                    ctx = ctx.entity("weapon", w);
                }
                if target == PLAYER {
                    self.world.learn(attacker);
                    self.msg("npc.attacks_you", &ctx);
                } else if attacker == PLAYER {
                    self.world.learn(target);
                    self.msg("attack.done", &ctx);
                    if self.world.is_invincible(target) {
                        self.msg("attack.invincible", &ctx);
                    }
                } else if self.world.room_of(attacker) == Some(here) {
                    self.msg("npc.attacks", &ctx);
                } else if let Some(room) = self.world.room_of(attacker) {
                    self.hear(room, "npc.fight_nearby");
                }
                // The whole faction thinks less of the player.
                if let Some(a) = attitude {
                    let words = a.faction.replace('_', " ");
                    let key = match (a.left <= 0, a.disguise.as_deref() == Some(a.faction.as_str())) {
                        (true, true) => "faction.sees_through",
                        (true, false) => "faction.turns",
                        (false, _) => "faction.wary",
                    };
                    self.msg(key, &Ctx::new().text("faction", words).entity("target", target));
                }
                // Patience wearing thin: worth a warning, if the player is there.
                if tolerated && self.world.room_of(target) == Some(here) && self.world.health(target).is_some_and(|h| h > 0) {
                    let key = if attacker == PLAYER { "npc.losing_patience_you" } else { "npc.losing_patience" };
                    self.msg(key, &Ctx::new().entity("target", target).entity("npc", attacker));
                }
            }
            NpcEvent::Fed { npc, food, room, finished } => {
                if room == here {
                    let key = if finished { "npc.finishes" } else { "npc.feeds" };
                    self.msg(key, &Ctx::new().entity("npc", npc).entity("target", food));
                } else {
                    self.hear(room, "npc.eating_nearby");
                }
            }
            NpcEvent::Discovered { npc, body, room, ref attitude } => {
                if room == here && !self.world.player_hidden() {
                    self.msg("npc.discovers", &Ctx::new().entity("npc", npc).entity("target", body));
                    if let Some(a) = attitude {
                        let words = a.faction.replace('_', " ");
                        let key = match (a.left <= 0, a.disguise.as_deref() == Some(a.faction.as_str())) {
                            (true, true) => "faction.sees_through",
                            (true, false) => "faction.turns",
                            (false, _) => "faction.wary",
                        };
                        self.msg(key, &Ctx::new().text("faction", words));
                    }
                } else {
                    self.hear(room, "npc.alarm_nearby");
                }
            }
            NpcEvent::Fetched { npc, item, room, brought } => {
                if room == here {
                    let key = if brought { "npc.brings" } else { "npc.grabs" };
                    self.msg(key, &Ctx::new().entity("npc", npc).entity("target", item));
                }
            }
            NpcEvent::Searched { npc, room } => {
                if room == here {
                    self.msg("npc.searches", &Ctx::new().entity("npc", npc));
                }
            }
            NpcEvent::Ate { npc, corpse, room } => {
                if room == here {
                    self.msg("npc.eats", &Ctx::new().entity("npc", npc).entity("target", corpse));
                } else {
                    self.hear(room, "npc.eating_nearby");
                }
            }
        }
    }

    /// Something in a neighbouring room can be heard, though not seen:
    /// at most one such message per turn.
    fn hear(&mut self, room: EntityId, key: &str) {
        let here = self.world.here();
        let Some(dir) = self.world.def(here).exits.iter().find(|e| e.to == Some(room)).map(|e| e.direction) else {
            return;
        };
        if self.heard_nearby == Some(self.world.state.turn) {
            return;
        }
        self.heard_nearby = Some(self.world.state.turn);
        let dir = self.grammar.direction_word(dir);
        self.msg(key, &Ctx::new().text("direction", dir));
    }

    /// Characters whose health has run out are defeated, and a player
    /// whose health has run out dies.
    pub(crate) fn check_defeats(&mut self) {
        let fallen: Vec<EntityId> = (0..self.world.story().entities.len() as u32)
            .map(EntityId)
            .filter(|&id| {
                self.world.def(id).kind == EntityKind::Character
                    && self.world.room_of(id).is_some()
                    && !self.world.is_dead(id)
                    && self.world.health(id).is_some_and(|h| h <= 0)
            })
            .collect();
        for id in fallen {
            self.defeat(id);
        }
        if self.world.health(PLAYER).is_some_and(|h| h <= 0) && self.end_requested.is_none() {
            for code in self.global_handlers(Phase::On, "died") {
                self.run(&code, &Ctx::new());
            }
            // A `died` handler may have saved the player, or ended the game its own way.
            if self.end_requested.is_none() && self.world.health(PLAYER).is_some_and(|h| h <= 0) {
                self.end_requested = Some("died".into());
            }
        }
    }

    /// Runs the character's `defeated` handlers. Unless one returns
    /// `block()`, "X is defeated." and it drops what it carries. Then,
    /// unless a handler healed it or moved it away, it becomes a corpse
    /// (`dead`), or with `corpse: false`, leaves the game.
    fn defeat(&mut self, id: EntityId) {
        let codes: Vec<Code> = self
            .world
            .def(id)
            .handlers
            .iter()
            .filter(|h| h.phase == Phase::On && h.event == "defeated")
            .map(|h| h.code.clone())
            .collect();
        let mut blocked = false;
        for code in codes {
            blocked |= self.run(&code, &Ctx::new().this(id)) == ScriptValue::Block;
        }
        // A handler may have healed it or moved it away.
        if self.world.room_of(id).is_none() || self.world.health(id).is_some_and(|h| h > 0) {
            return;
        }
        let room = self.world.room_of(id);
        if !blocked {
            let seen = room == Some(self.world.here());
            if seen {
                self.msg("npc.defeated", &Ctx::new().entity("target", id));
            }
            let items = self.world.contents(id);
            for &item in &items {
                self.world.unset(item, "worn");
                let _ = self.world.move_to(item, room);
            }
            if seen && !items.is_empty() {
                let list = self.list_a(&items);
                self.msg("npc.drops", &Ctx::new().entity("target", id).text("list", list));
            }
        }
        if self.world.def(id).npc.as_ref().is_none_or(|n| n.corpse) {
            self.world.set(id, "dead", Value::Bool(true));
            self.world.learn(id);
        } else {
            let _ = self.world.move_to(id, None);
        }
    }

    /// How a body came to be one: "The goblin chief has been torn apart
    /// by teeth." The words come from the weapon's `kills`, else the
    /// killer's, else the killer's faction's.
    pub(crate) fn kill_story(&mut self, id: EntityId) -> Option<String> {
        let key = |w: &crate::World, field: &str| match w.get(id, field) {
            Some(Value::Str(k)) => w.id(k),
            _ => None,
        };
        let killer = key(&self.world, "killed_by")?;
        let weapon = key(&self.world, "killed_with");
        let how = weapon
            .and_then(|w| self.world.def(w).kills.clone())
            .or_else(|| self.world.def(killer).kills.clone())
            .or_else(|| self.world.faction(killer).and_then(|f| f.kills.clone()));
        let ctx = Ctx::new().entity("target", id).entity("by", killer);
        Some(match how {
            Some(how) => self.msg_str("state.killed_how", &ctx.text("how", how)),
            None if killer == PLAYER => self.msg_str("state.killed_by_you", &ctx),
            None => self.msg_str("state.killed_by", &ctx),
        })
    }

    pub(crate) fn apologize_action(&mut self, t: EntityId, ctx: &Ctx) -> bool {
        use crate::npc::Apology;
        let (key, calmed) = match self.world.apologize(t) {
            Apology::NotPerson => ("apologize.not_person", None),
            Apology::Nothing => ("apologize.nothing", None),
            Apology::Refused => ("apologize.refused", None),
            Apology::Exhausted => ("apologize.exhausted", None),
            Apology::Accepted { calmed } => ("apologize.accepted", calmed),
        };
        self.msg(key, ctx);
        if let Some(f) = &calmed {
            self.msg("faction.calmer", &Ctx::new().text("faction", f.replace('_', " ")));
        }
        key == "apologize.accepted"
    }

    /// THROW X (AT Y / DIRECTION). At something that can be hurt, it's an
    /// attack with X as the weapon; X then lands in the room. In a
    /// direction, X lands in the next room that way (over an exit the
    /// player may not be able to walk through, but not through a closed
    /// door), making a small noise there. Companions that fetch go after it.
    pub(crate) fn throw_action(
        &mut self,
        t: eifgb_ir::EntityId,
        at: Option<EntityId>,
        dir: Option<eifgb_ir::Direction>,
        ctx: &Ctx,
    ) -> bool {
        if self.world.location(t) != Some(PLAYER) {
            self.msg("throw.not_held", ctx);
            return false;
        }
        if self.world.weight(t) > 3 {
            self.msg("throw.too_heavy", ctx);
            return false;
        }
        let here = self.world.here();
        if let Some(y) = at {
            if y == PLAYER {
                self.msg("attack.self", ctx);
                return false;
            }
            self.world.unset(t, "worn");
            let _ = self.world.move_to(t, Some(here));
            self.msg("throw.at", ctx);
            let hurtable = !self.world.is_dead(y) && (self.world.health(y).is_some() || self.world.is_invincible(y));
            match hurtable.then(|| self.world.attack(PLAYER, y, Some(t))).flatten() {
                Some(event) => self.narrate(&event),
                None => self.msg("throw.bounces", ctx),
            }
        } else if let Some(d) = dir {
            let exit = self.world.def(here).exits.iter().find(|e| e.direction == d && e.to.is_some() && self.world.exit_visible(e)).cloned();
            let Some(exit) = exit else {
                self.no_turn = true;
                self.msg("throw.no_way", ctx);
                return false;
            };
            if let Some(door) = exit.through
                && !self.world.is_open(door)
            {
                self.msg("throw.door", &ctx.clone().entity("door", door));
                return false;
            }
            let to = exit.to.unwrap_or(here);
            self.world.unset(t, "worn");
            let _ = self.world.move_to(t, Some(to));
            let words = self.grammar.direction_word(d);
            self.msg("throw.direction", &ctx.clone().text("direction", words));
            self.world.make_noise(to, 1);
        } else {
            self.world.unset(t, "worn");
            let _ = self.world.move_to(t, Some(here));
            self.msg("throw.away", ctx);
        }
        for f in self.world.send_fetchers(t) {
            if self.world.together(f, PLAYER) {
                self.msg("npc.fetches", &Ctx::new().entity("npc", f).entity("target", t));
            }
        }
        true
    }

    /// BURY X: in soft ground, it's gone for good. Bodies most of all.
    pub(crate) fn bury_action(&mut self, t: EntityId, ctx: &Ctx) -> bool {
        let here = self.world.here();
        if !self.world.def(here).diggable {
            self.msg("bury.cant", ctx);
            return false;
        }
        let movable = self.world.is_dead(t) || self.world.has(t, eifgb_ir::Trait::Portable);
        if !movable || self.world.room_of(t) != Some(here) {
            self.msg("bury.not_that", ctx);
            return false;
        }
        self.world.unset(t, "worn");
        let _ = self.world.move_to(t, None);
        self.msg("bury.done", ctx);
        true
    }

    /// CALL X Y: names something `nameable`.
    pub(crate) fn rename_action(&mut self, t: EntityId, text: Option<&str>, ctx: &Ctx) -> bool {
        if !self.world.def(t).nameable {
            self.msg("name.cant", ctx);
            return false;
        }
        let Some(name) = text.and_then(World::clean_name) else {
            self.no_turn = true;
            self.msg("name.bad", ctx);
            return false;
        };
        let old = self.the(t);
        self.world.rename(t, &name);
        self.msg("name.done", &ctx.clone().text("old", old).text("name", name));
        true
    }

    /// GIVE X TO Y.
    pub(crate) fn give_action(&mut self, item: EntityId, to: Option<EntityId>, ctx: &Ctx) -> bool {
        use crate::npc::Gift;
        let Some(to) = to else {
            // Just a question back: it doesn't take a turn.
            self.no_turn = true;
            self.msg("give.whom", ctx);
            return false;
        };
        let (key, calmed) = match self.world.give(item, to) {
            Gift::NotHeld => ("give.not_held", None),
            Gift::NotPerson => ("give.not_person", None),
            Gift::Unwanted => ("give.unwanted", None),
            Gift::Tamed { eaten: true } => ("give.tamed_eaten", None),
            Gift::Tamed { eaten: false } => ("give.tamed", None),
            Gift::Liked { calmed } => ("give.liked", calmed),
        };
        self.msg(key, ctx);
        if let Some(f) = calmed {
            self.msg("faction.calmer", &Ctx::new().text("faction", f.replace('_', " ")));
        }
        matches!(key, "give.tamed" | "give.tamed_eaten" | "give.liked")
    }

    /// GIVE 5 COINS TO GUARD, PAY GUARD 5, BRIBE GUARD WITH 5.
    pub(crate) fn bribe_action(&mut self, npc: EntityId, text: Option<&str>, ctx: &Ctx) -> bool {
        use crate::npc::Bribe;
        let Some(amount) = text.and_then(first_number) else {
            // Just a question back: it doesn't take a turn.
            self.no_turn = true;
            self.msg("bribe.how_much", ctx);
            return false;
        };
        let price = self.money_text(amount);
        let have = self.money_text(self.world.money(PLAYER));
        let key = match self.world.bribe(npc, amount) {
            Bribe::NotPerson => "give.not_person_money",
            Bribe::Refused => "bribe.refused",
            Bribe::Already => "bribe.already",
            Bribe::CantAfford => "bribe.cant_afford",
            Bribe::TooLittle => "bribe.too_little",
            Bribe::Taken => "bribe.taken",
        };
        self.msg(key, &ctx.clone().text("price", price).text("amount", have));
        key == "bribe.taken"
    }

    /// "12 coins"
    pub(crate) fn money_text(&mut self, amount: i64) -> String {
        self.msg_str("money.amount", &Ctx::new().value("amount", Value::Int(amount)))
    }

    // --------------------------------------------------------------- fighting

    pub(crate) fn attack_action(&mut self, t: EntityId, tool: Option<EntityId>, ctx: &Ctx) -> bool {
        if t == PLAYER {
            self.msg("attack.self", ctx);
            return false;
        }
        if let Some(w) = tool {
            if !self.world.is_within(w, PLAYER) {
                self.msg("attack.not_held", ctx);
                return false;
            }
            if self.world.def(w).damage.is_none() {
                self.msg("attack.not_weapon", ctx);
                return false;
            }
        }
        if self.world.is_dead(t) {
            self.msg("attack.dead", ctx);
            return false;
        }
        if self.world.health(t).is_none() && !self.world.is_invincible(t) {
            self.msg("attack.pointless", ctx);
            return false;
        }
        match self.world.attack(PLAYER, t, tool) {
            Some(event) => {
                self.narrate(&event);
                true
            }
            None => false,
        }
    }

    /// Doesn't take a turn: checking on yourself shouldn't give an enemy a free hit.
    pub(crate) fn health_action(&mut self) -> bool {
        self.no_turn = true;
        let max = self.world.def(PLAYER).health;
        match (self.world.health(PLAYER), max) {
            (Some(h), Some(m)) if h >= i64::from(m) => self.msg("health.full", &Ctx::new()),
            (Some(h), Some(m)) => self.msg(
                "health.hurt",
                &Ctx::new().value("health", Value::Int(h)).value("max", Value::Int(i64::from(m))),
            ),
            _ => self.msg("health.none", &Ctx::new()),
        }
        true
    }

    // ---------------------------------------------------------------- trading

    /// The merchant named as the second object, or else the first one here.
    fn pick_merchant(&self, named: Option<EntityId>) -> Option<EntityId> {
        let here = self.world.merchants_here();
        match named {
            Some(m) => here.contains(&m).then_some(m),
            None => here.first().copied(),
        }
    }

    /// The merchant here selling `t`, and its asking price, or the reason
    /// it can't be bought (already reported).
    fn seller_of(&mut self, t: EntityId, merchant: Option<EntityId>, ctx: &Ctx) -> Option<(EntityId, i64)> {
        let merchants = self.world.merchants_here();
        if merchants.is_empty() || merchant.is_some_and(|m| !merchants.contains(&m)) {
            self.msg("buy.no_merchant", ctx);
            return None;
        }
        let seller = merchants.iter().copied().find(|&m| self.world.location(t) == Some(m) && merchant.is_none_or(|n| n == m));
        let (Some(seller), Some(price)) = (seller, self.world.def(t).price) else {
            self.msg("buy.not_for_sale", ctx);
            return None;
        };
        if self.refuses_trade(seller, ctx) {
            return None;
        }
        Some((seller, i64::from(price)))
    }

    /// A merchant pushed too far won't trade with the player at all.
    fn refuses_trade(&mut self, merchant: EntityId, ctx: &Ctx) -> bool {
        if self.world.flag(merchant, "_refuses") {
            self.world.unset(merchant, "_deal");
            self.msg("trade.refused", &ctx.clone().entity("merchant", merchant));
            return true;
        }
        false
    }

    pub(crate) fn buy(&mut self, t: EntityId, merchant: Option<EntityId>, ctx: &Ctx) -> bool {
        let Some((seller, price)) = self.seller_of(t, merchant, ctx) else { return false };
        let have = self.world.money(PLAYER);
        if have < price {
            let ctx = ctx.clone().text("price", self.money_text(price)).text("amount", self.money_text(have));
            self.msg("buy.too_poor", &ctx);
            return false;
        }
        self.purchase(t, seller, price, "buy.done", ctx)
    }

    /// Hands `t` over for `price`, if the player can carry it.
    fn purchase(&mut self, t: EntityId, seller: EntityId, price: i64, key: &str, ctx: &Ctx) -> bool {
        let refusal = match self.world.fits(t, PLAYER) {
            Fit::Yes => None,
            Fit::TooBig => Some("take.too_big"),
            Fit::TooHeavy => Some("take.too_heavy"),
            Fit::Full => Some("take.full"),
        };
        if let Some(k) = refusal {
            self.msg(k, ctx);
            return false;
        }
        let have = self.world.money(PLAYER);
        self.world.set_money(PLAYER, have - price);
        if self.world.has_money(seller) {
            let m = self.world.money(seller);
            self.world.set_money(seller, m + price);
        }
        let _ = self.world.move_to(t, Some(PLAYER));
        self.world.learn(t);
        let ctx = ctx.clone().entity("merchant", seller).text("price", self.money_text(price));
        self.msg(key, &ctx);
        true
    }

    /// OFFER 5 FOR LAMP. The asking price or more buys it. Less is accepted
    /// down to the merchant's floor (`haggle` percent off); below that, the
    /// merchant counters, and each lowball offer costs a point of patience.
    /// Out of patience, the merchant refuses to trade at all.
    pub(crate) fn offer(&mut self, t: EntityId, text: Option<&str>, ctx: &Ctx) -> bool {
        let Some((seller, price)) = self.seller_of(t, None, ctx) else { return false };
        let ctx = ctx.clone().entity("merchant", seller);
        let amount = text.and_then(first_number);
        let Some(amount) = amount else {
            // Just a question back: it doesn't take a turn.
            self.no_turn = true;
            self.msg("offer.how_much", &ctx);
            return false;
        };
        if amount > self.world.money(PLAYER) {
            let have = self.money_text(self.world.money(PLAYER));
            self.msg("offer.cant_afford", &ctx.text("amount", have));
            return false;
        }
        if amount >= price {
            return self.purchase(t, seller, price, "buy.done", &ctx);
        }
        let haggle = self.world.def(seller).npc.as_ref().map_or(0, |n| n.haggle);
        let price_text = self.money_text(price);
        if haggle == 0 {
            self.msg("offer.fixed", &ctx.text("price", price_text));
            return false;
        }
        // The least it'll take, rounded up.
        let floor = (price * i64::from(100 - haggle.min(100)) + 99) / 100;
        if amount >= floor {
            return self.purchase(t, seller, amount, "offer.accepted", &ctx);
        }
        let patience = match self.world.get(seller, "patience") {
            Some(Value::Int(i)) => *i,
            _ => 0,
        };
        // Like being attacked: it puts up with `patience` lowball offers.
        if patience <= 0 {
            self.world.set(seller, "_refuses", Value::Bool(true));
            self.msg("offer.insulted", &ctx);
            return false;
        }
        self.world.set(seller, "patience", Value::Int(patience - 1));
        // "…and not a coin less": the counter is the least it'll take.
        let counter = floor;
        self.set_deal(seller, "buy", t, counter);
        let counter = self.money_text(counter);
        self.msg("offer.counter", &ctx.text("price", counter));
        false
    }

    /// SELL X (TO M) (FOR N). A merchant who haggles names its price first,
    /// so the player knows they can ask for more; SELL X again (or ACCEPT)
    /// takes it. FOR N asks for a price: anything up to `haggle` percent
    /// above the merchant's offer (never above its own selling price) is
    /// accepted; more gets a counter-offer and costs patience.
    pub(crate) fn sell(&mut self, t: EntityId, merchant: Option<EntityId>, text: Option<&str>, ctx: &Ctx) -> bool {
        let Some(buyer) = self.pick_merchant(merchant) else {
            self.msg("sell.no_merchant", ctx);
            return false;
        };
        if self.refuses_trade(buyer, ctx) {
            return false;
        }
        let ctx = ctx.clone().entity("merchant", buyer);
        if self.world.location(t) != Some(PLAYER) {
            self.msg("sell.not_held", &ctx);
            return false;
        }
        let n = self.world.def(buyer).npc.clone();
        let (buys_at, haggle) = n.as_ref().map_or((0, 0), |n| (n.buys_at, n.haggle));
        let price = self.world.def(t).price.map_or(0, i64::from);
        let offer = price * i64::from(buys_at) / 100;
        if offer <= 0 {
            self.msg("sell.worthless", &ctx);
            return false;
        }
        let purse = self.world.has_money(buyer).then(|| self.world.money(buyer));
        if purse.is_some_and(|m| m < offer) {
            self.msg("sell.merchant_poor", &ctx);
            return false;
        }
        let asked = text.and_then(first_number);
        if text.is_some() && asked.is_none() {
            // Just a question back: it doesn't take a turn.
            self.no_turn = true;
            self.msg("sell.how_much", &ctx);
            return false;
        }
        // Fixed prices: just sell, at the price shown.
        if haggle == 0 {
            if asked.is_some_and(|a| a > offer) {
                let o = self.money_text(offer);
                self.msg("sell.fixed", &ctx.text("price", o));
                return false;
            }
            return self.complete_sale(t, buyer, asked.unwrap_or(offer), "sell.done", &ctx);
        }
        let Some(asked) = asked else {
            // A deal on the table for this: SELL again takes it.
            if let Some(p) = self.deal(buyer, "sell", t) {
                return self.complete_sale(t, buyer, p, "sell.done", &ctx);
            }
            self.set_deal(buyer, "sell", t, offer);
            let o = self.money_text(offer);
            let example = offer + (offer / 5).max(1);
            self.msg("sell.offer", &ctx.text("price", o).value("example", Value::Int(example)));
            return false;
        };
        // The most it'll pay, rounded down: never more than its own price,
        // or than it has.
        let ceiling = (offer * i64::from(100 + haggle) / 100).min(price.max(offer)).min(purse.unwrap_or(i64::MAX));
        if asked <= ceiling {
            let key = if asked > offer { "sell.accepted" } else { "sell.done" };
            return self.complete_sale(t, buyer, asked, key, &ctx);
        }
        let patience = match self.world.get(buyer, "patience") {
            Some(Value::Int(i)) => *i,
            _ => 0,
        };
        if patience <= 0 {
            self.world.set(buyer, "_refuses", Value::Bool(true));
            self.world.unset(buyer, "_deal");
            self.msg("offer.insulted", &ctx);
            return false;
        }
        self.world.set(buyer, "patience", Value::Int(patience - 1));
        // "…and not a coin more": the counter is the most it'll pay.
        let counter = ceiling;
        self.set_deal(buyer, "sell", t, counter);
        let c = self.money_text(counter);
        self.msg("sell.counter", &ctx.text("price", c));
        false
    }

    fn complete_sale(&mut self, t: EntityId, buyer: EntityId, price: i64, key: &str, ctx: &Ctx) -> bool {
        if self.world.has_money(buyer) {
            let m = self.world.money(buyer);
            self.world.set_money(buyer, m - price);
        }
        let have = self.world.money(PLAYER);
        self.world.set_money(PLAYER, have + price);
        self.world.unset(t, "worn");
        self.world.unset(buyer, "_deal");
        let _ = self.world.move_to(t, Some(buyer));
        let p = self.money_text(price);
        self.msg(key, &ctx.clone().entity("merchant", buyer).text("price", p));
        true
    }

    /// A deal on the table: the merchant's last offer or counter-offer,
    /// which ACCEPT (or repeating SELL) takes.
    fn set_deal(&mut self, merchant: EntityId, kind: &str, item: EntityId, price: i64) {
        let mut m = std::collections::BTreeMap::new();
        m.insert("kind".to_string(), Value::Str(kind.into()));
        m.insert("item".to_string(), Value::Str(self.world.def(item).key.clone()));
        m.insert("price".to_string(), Value::Int(price));
        self.world.set(merchant, "_deal", Value::Map(m));
    }

    /// The price of a deal of this kind on the table for `item`.
    fn deal(&self, merchant: EntityId, kind: &str, item: EntityId) -> Option<i64> {
        let Some(Value::Map(m)) = self.world.get(merchant, "_deal") else { return None };
        let same = m.get("kind") == Some(&Value::Str(kind.into()))
            && m.get("item") == Some(&Value::Str(self.world.def(item).key.clone()));
        match m.get("price") {
            Some(Value::Int(p)) if same => Some(*p),
            _ => None,
        }
    }

    /// ACCEPT: takes the deal a merchant here has on the table.
    pub(crate) fn accept(&mut self, ctx: &Ctx) -> bool {
        for m in self.world.merchants_here() {
            let Some(Value::Map(d)) = self.world.get(m, "_deal").cloned() else { continue };
            let (Some(Value::Str(kind)), Some(Value::Str(item)), Some(Value::Int(price))) =
                (d.get("kind"), d.get("item"), d.get("price"))
            else {
                continue;
            };
            let Some(item) = self.world.id(item) else { continue };
            let ctx = ctx.clone().entity("target", item).entity("merchant", m);
            if self.refuses_trade(m, &ctx) {
                return false;
            }
            return match kind.as_str() {
                // Taking the merchant's own price: a plain sale.
                "sell" if self.world.location(item) == Some(PLAYER) => self.complete_sale(item, m, *price, "sell.done", &ctx),
                "buy" if self.world.location(item) == Some(m) => {
                    if self.world.money(PLAYER) < *price {
                        let have = self.money_text(self.world.money(PLAYER));
                        self.msg("offer.cant_afford", &ctx.text("amount", have));
                        return false;
                    }
                    self.world.unset(m, "_deal");
                    self.purchase(item, m, *price, "buy.done", &ctx)
                }
                _ => {
                    self.world.unset(m, "_deal");
                    self.msg("accept.nothing", &ctx);
                    false
                }
            };
        }
        self.msg("accept.nothing", ctx);
        false
    }

    pub(crate) fn wares(&mut self, merchant: Option<EntityId>, ctx: &Ctx) -> bool {
        let merchants: Vec<EntityId> = match merchant {
            Some(m) if self.world.def(m).npc.as_ref().is_some_and(|n| n.behavior == Behavior::Merchant) => vec![m],
            Some(_) => Vec::new(),
            None => self.world.merchants_here(),
        };
        if merchants.is_empty() {
            self.msg("buy.no_merchant", ctx);
            return false;
        }
        for m in merchants {
            if self.refuses_trade(m, ctx) {
                continue;
            }
            let goods: Vec<EntityId> =
                self.world.contents(m).into_iter().filter(|&i| self.world.def(i).price.is_some()).collect();
            let ctx = Ctx::new().entity("merchant", m);
            if goods.is_empty() {
                self.msg("wares.empty", &ctx);
                continue;
            }
            let items: Vec<String> = goods
                .iter()
                .map(|&i| {
                    let price = self.money_text(i64::from(self.world.def(i).price.unwrap_or(0)));
                    self.msg_str("wares.item", &Ctx::new().text("item", self.a(i)).text("price", price))
                })
                .collect();
            let list = (self.grammar.lang().list_and)(&items);
            self.msg("wares.list", &ctx.text("list", list));
            for g in goods {
                self.world.learn(g);
            }
        }
        true
    }

    /// A line for the inventory: "You have 12 coins.", if the player has money at all.
    pub(crate) fn report_money(&mut self) {
        if self.world.has_money(PLAYER) {
            let amount = self.money_text(self.world.money(PLAYER));
            self.msg_kind(LineKind::Text, "money.have", &Ctx::new().text("amount", amount));
        }
    }
}

/// The first whole number in some text: "6", "6 coins", "for 6".
fn first_number(text: &str) -> Option<i64> {
    text.split(|c: char| !c.is_ascii_digit()).find(|w| !w.is_empty()).and_then(|w| w.parse().ok())
}
