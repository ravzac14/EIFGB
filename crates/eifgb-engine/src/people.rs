//! Characters in the turn loop: their turns, fighting, and trading.
//!
//! [`crate::npc`] decides what characters do and changes the world. This
//! module runs their turns, describes what the player can see, and
//! carries out ATTACK, BUY, SELL, WARES and HEALTH.

use crate::game::{Game, LineKind};
use crate::npc::NpcEvent;
use crate::script::{Ctx, ScriptHost, ScriptValue};
use crate::world::Fit;
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
            NpcEvent::Attacked { attacker, target, weapon, .. } => {
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
                } else if self.world.room_of(attacker) == Some(here) {
                    self.msg("npc.attacks", &ctx);
                } else if let Some(room) = self.world.room_of(attacker) {
                    self.hear(room, "npc.fight_nearby");
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
        if self.world.health(t).is_none() {
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

    pub(crate) fn buy(&mut self, t: EntityId, merchant: Option<EntityId>, ctx: &Ctx) -> bool {
        let merchants = self.world.merchants_here();
        if merchants.is_empty() || merchant.is_some_and(|m| !merchants.contains(&m)) {
            self.msg("buy.no_merchant", ctx);
            return false;
        }
        let seller = merchants.iter().copied().find(|&m| self.world.location(t) == Some(m) && merchant.is_none_or(|n| n == m));
        let (Some(seller), Some(price)) = (seller, self.world.def(t).price) else {
            self.msg("buy.not_for_sale", ctx);
            return false;
        };
        let price = i64::from(price);
        let have = self.world.money(PLAYER);
        if have < price {
            let ctx = ctx.clone().text("price", self.money_text(price)).text("amount", self.money_text(have));
            self.msg("buy.too_poor", &ctx);
            return false;
        }
        match self.world.fits(t, PLAYER) {
            Fit::Yes => {}
            Fit::TooBig => {
                self.msg("take.too_big", ctx);
                return false;
            }
            Fit::TooHeavy => {
                self.msg("take.too_heavy", ctx);
                return false;
            }
            Fit::Full => {
                self.msg("take.full", ctx);
                return false;
            }
        }
        self.world.set_money(PLAYER, have - price);
        if self.world.has_money(seller) {
            let m = self.world.money(seller);
            self.world.set_money(seller, m + price);
        }
        let _ = self.world.move_to(t, Some(PLAYER));
        self.world.learn(t);
        let ctx = ctx.clone().entity("merchant", seller).text("price", self.money_text(price));
        self.msg("buy.done", &ctx);
        true
    }

    pub(crate) fn sell(&mut self, t: EntityId, merchant: Option<EntityId>, ctx: &Ctx) -> bool {
        let Some(buyer) = self.pick_merchant(merchant) else {
            self.msg("sell.no_merchant", ctx);
            return false;
        };
        let ctx = ctx.clone().entity("merchant", buyer);
        if self.world.location(t) != Some(PLAYER) {
            self.msg("sell.not_held", &ctx);
            return false;
        }
        let buys_at = self.world.def(buyer).npc.as_ref().map_or(0, |n| n.buys_at);
        let offer = self.world.def(t).price.map_or(0, |p| i64::from(p) * i64::from(buys_at) / 100);
        if offer <= 0 {
            self.msg("sell.worthless", &ctx);
            return false;
        }
        if self.world.has_money(buyer) && self.world.money(buyer) < offer {
            self.msg("sell.merchant_poor", &ctx);
            return false;
        }
        if self.world.has_money(buyer) {
            let m = self.world.money(buyer);
            self.world.set_money(buyer, m - offer);
        }
        let have = self.world.money(PLAYER);
        self.world.set_money(PLAYER, have + offer);
        self.world.unset(t, "worn");
        let _ = self.world.move_to(t, Some(buyer));
        let price = self.money_text(offer);
        self.msg("sell.done", &ctx.text("price", price));
        true
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
