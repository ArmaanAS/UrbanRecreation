//! Exact, information-aware continuation values for the strict diagnostic engine.
//!
//! The engine resolves one complete round at a time, while the live game reveals the
//! first mover's card but not its pillz or Fury.  When the opponent moves first, one of
//! our responses therefore has to cover every hidden wager for that visible card.  The
//! response may change when the visible card changes.  When we move first, the opponent
//! is conservatively allowed to answer our complete move.
//!
//! This module only evaluates an already prepared [`CombatStatDiagnosticV1`].  It does
//! not widen that engine's admitted effect set or turn an unsupported effect into a no-op.

use std::time::{Duration, Instant};

use crate::engine::{
    BaseRulesRoundInput, BaseRulesSelection, ByPlayer, CombatStatDiagnosticV1, MatchStatus,
    PlayerId, FURY_COST, HAND_SIZE,
};

/// An exact terminal result in the asking player's frame.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ExactValue {
    Loss,
    Draw,
    Win,
}

/// Deadline and diagnostic work count shared by one continuation search.
#[derive(Clone, Debug)]
pub struct PolicyControl {
    // An unrepresentably distant deadline is equivalent to no deadline.
    deadline: Option<Instant>,
    nodes: u64,
    // Deterministic nested-cancellation coverage without adding a production policy knob.
    #[cfg(test)]
    node_limit: Option<u64>,
}

impl PolicyControl {
    pub fn until(deadline: Instant) -> Self {
        Self {
            deadline: Some(deadline),
            nodes: 0,
            #[cfg(test)]
            node_limit: None,
        }
    }

    /// Build a control from the same start/budget pair used by a root search.
    ///
    /// `Instant` has a platform-dependent finite range.  An overflowing duration means
    /// the requested deadline cannot occur during this process, so it is represented as
    /// no deadline instead of panicking.
    pub fn for_budget(started: Instant, budget: Duration) -> Self {
        Self {
            deadline: started.checked_add(budget),
            nodes: 0,
            #[cfg(test)]
            node_limit: None,
        }
    }

    /// Complete round inputs executed so far.
    pub const fn nodes(&self) -> u64 {
        self.nodes
    }

    fn before_make(&mut self) -> bool {
        if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            return false;
        }
        #[cfg(test)]
        if self.node_limit.is_some_and(|limit| self.nodes >= limit) {
            return false;
        }
        self.nodes += 1;
        true
    }

    #[cfg(test)]
    pub(crate) fn for_nodes(node_limit: u64) -> Self {
        Self {
            deadline: Instant::now().checked_add(Duration::from_secs(60)),
            nodes: 0,
            node_limit: Some(node_limit),
        }
    }
}

/// Conservative eventual result under a pure policy that respects hidden pillz and Fury.
///
/// `next_first_mover` is explicit because it is context beside the engine position and
/// cannot be reconstructed from round parity.  `None` means the deadline was reached.
/// The game is restored exactly on every return path, including nested cancellation.
pub fn continuation_value(
    game: &mut CombatStatDiagnosticV1,
    us: PlayerId,
    next_first_mover: PlayerId,
    control: &mut PolicyControl,
) -> Option<ExactValue> {
    terminal_value(game, us)
        .map(Some)
        .unwrap_or_else(|| round_value(game, us, next_first_mover, control))
}

fn round_value(
    game: &mut CombatStatDiagnosticV1,
    us: PlayerId,
    first_mover: PlayerId,
    control: &mut PolicyControl,
) -> Option<ExactValue> {
    if first_mover == us {
        our_first_value(game, us, first_mover, control)
    } else {
        opponent_first_value(game, us, first_mover, control)
    }
}

/// We choose a complete move; pessimistically, the opponent sees it and finds its worst
/// complete reply.
fn our_first_value(
    game: &mut CombatStatDiagnosticV1,
    us: PlayerId,
    first_mover: PlayerId,
    control: &mut PolicyControl,
) -> Option<ExactValue> {
    let opponent = us.other();
    let mut best = ExactValue::Loss;

    for our_action in legal_actions(game, us) {
        let mut reply_worst = ExactValue::Win;
        for opponent_action in legal_actions(game, opponent) {
            let value =
                evaluate_round(game, us, first_mover, our_action, opponent_action, control)?;
            reply_worst = reply_worst.min(value);
            // Exact endpoint pruning only.  No wager-domination assumptions are valid in
            // the presence of Defeat and other non-monotone effects.
            if reply_worst == ExactValue::Loss {
                break;
            }
        }
        best = best.max(reply_worst);
        if best == ExactValue::Win {
            break;
        }
    }
    Some(best)
}

/// The opponent chooses a card and hidden wager.  Our response may depend on the visible
/// card, but that one response must survive every pillz/Fury hypothesis for the card.
fn opponent_first_value(
    game: &mut CombatStatDiagnosticV1,
    us: PlayerId,
    first_mover: PlayerId,
    control: &mut PolicyControl,
) -> Option<ExactValue> {
    let opponent = us.other();
    let opponent_played = game.position().played[opponent];
    let opponent_pillz = game.position().players[opponent].pillz;
    let mut result = ExactValue::Win;

    for opponent_slot in 0..HAND_SIZE {
        if opponent_played[opponent_slot] {
            continue;
        }

        let mut best_response = ExactValue::Loss;
        for our_action in legal_actions(game, us) {
            let mut response_worst = ExactValue::Win;
            for wager in Wagers::new(opponent_pillz) {
                let opponent_action =
                    BaseRulesSelection::new(opponent_slot as u8, wager.pillz, wager.fury);
                let value =
                    evaluate_round(game, us, first_mover, our_action, opponent_action, control)?;
                response_worst = response_worst.min(value);
                if response_worst == ExactValue::Loss {
                    break;
                }
            }
            best_response = best_response.max(response_worst);
            if best_response == ExactValue::Win {
                break;
            }
        }

        result = result.min(best_response);
        if result == ExactValue::Loss {
            break;
        }
    }
    Some(result)
}

fn evaluate_round(
    game: &mut CombatStatDiagnosticV1,
    us: PlayerId,
    first_mover: PlayerId,
    our_action: BaseRulesSelection,
    opponent_action: BaseRulesSelection,
    control: &mut PolicyControl,
) -> Option<ExactValue> {
    if !control.before_make() {
        return None;
    }
    let selections = match us {
        PlayerId::P1 => ByPlayer::new(our_action, opponent_action),
        PlayerId::P2 => ByPlayer::new(opponent_action, our_action),
    };
    let (_, undo) = game
        .make(BaseRulesRoundInput {
            first_mover,
            selections,
        })
        .expect("legal policy action must execute in a fully admitted diagnostic match");

    let child = terminal_value(game, us)
        .map(Some)
        .unwrap_or_else(|| round_value(game, us, first_mover.other(), control));
    // Never use `?` above this point: cancellation must walk the mutation back first.
    game.unmake(undo);
    child
}

fn terminal_value(game: &CombatStatDiagnosticV1, us: PlayerId) -> Option<ExactValue> {
    match game.position().status {
        MatchStatus::Playing => None,
        MatchStatus::Won(winner) if winner == us => Some(ExactValue::Win),
        MatchStatus::Won(_) => Some(ExactValue::Loss),
        MatchStatus::Draw => Some(ExactValue::Draw),
    }
}

/// Stack-only iterator over all legal card/wager actions for one player.
#[derive(Clone, Copy, Debug)]
struct LegalActions {
    played: [bool; HAND_SIZE],
    available: u16,
    slot: usize,
    wagers: Wagers,
}

impl LegalActions {
    fn new(played: [bool; HAND_SIZE], available: u16) -> Self {
        Self {
            played,
            available,
            slot: 0,
            wagers: Wagers::new(available),
        }
    }
}

impl Iterator for LegalActions {
    type Item = BaseRulesSelection;

    fn next(&mut self) -> Option<Self::Item> {
        while self.slot < HAND_SIZE {
            if self.played[self.slot] {
                self.slot += 1;
                self.wagers = Wagers::new(self.available);
                continue;
            }
            if let Some(wager) = self.wagers.next() {
                return Some(BaseRulesSelection::new(
                    self.slot as u8,
                    wager.pillz,
                    wager.fury,
                ));
            }
            self.slot += 1;
            self.wagers = Wagers::new(self.available);
        }
        None
    }
}

fn legal_actions(game: &CombatStatDiagnosticV1, player: PlayerId) -> LegalActions {
    let position = game.position();
    LegalActions::new(position.played[player], position.players[player].pillz)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Wager {
    pillz: u16,
    fury: bool,
}

/// The reference probe order without allocating `ordered_bets`: plain all-in, largest
/// Fury wager, cheap probes, then the two near-all-in plain wagers.
#[derive(Clone, Copy, Debug)]
struct Wagers {
    available: u16,
    paid_ordinal: u32,
    fury_emitted: bool,
}

impl Wagers {
    const fn new(available: u16) -> Self {
        Self {
            available,
            paid_ordinal: 0,
            fury_emitted: false,
        }
    }

    fn paid_at(self, ordinal: u32) -> u16 {
        let available = u32::from(self.available);
        let paid = if ordinal == 0 {
            available
        } else if available < u32::from(FURY_COST) {
            ordinal - 1
        } else if ordinal == 1 {
            available - u32::from(FURY_COST)
        } else if ordinal <= available - 2 {
            ordinal - 2
        } else if ordinal == available - 1 {
            available - 2
        } else {
            available - 1
        };
        paid as u16
    }
}

impl Iterator for Wagers {
    type Item = Wager;

    fn next(&mut self) -> Option<Self::Item> {
        if self.paid_ordinal > u32::from(self.available) {
            return None;
        }
        let paid = self.paid_at(self.paid_ordinal);
        let fury_allowed = self.available >= FURY_COST && paid <= self.available - FURY_COST;
        if fury_allowed && !self.fury_emitted {
            self.fury_emitted = true;
            return Some(Wager {
                pillz: paid,
                fury: true,
            });
        }
        self.fury_emitted = false;
        self.paid_ordinal += 1;
        Some(Wager {
            pillz: paid,
            fury: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::catalog::CardKey;
    use crate::engine::{
        BaseRulesCardSpec, BaseRulesMatchSpec, BaseRulesPlayerSpec, CombatStatCardPlanV1,
        CombatStatDiagnosticMatchSpecV1, CombatStatSourcePlanV1,
    };

    type Stats = [(u16, u16); HAND_SIZE];

    fn test_game(life: u16, pillz: u16, p1: Stats, p2: Stats) -> CombatStatDiagnosticV1 {
        let hand = |base: u32, stats: Stats| {
            std::array::from_fn(|slot| BaseRulesCardSpec {
                key: CardKey::new(base + slot as u32, 1),
                clan_id: base + slot as u32,
                power: stats[slot].0,
                damage: stats[slot].1,
            })
        };
        let base_rules = BaseRulesMatchSpec {
            battle_rule_id: 0,
            night: false,
            players: ByPlayer::new(
                BaseRulesPlayerSpec {
                    initial_life: life,
                    initial_pillz: pillz,
                    hand: hand(100, p1),
                },
                BaseRulesPlayerSpec {
                    initial_life: life,
                    initial_pillz: pillz,
                    hand: hand(200, p2),
                },
            ),
        };
        let plan = |card: BaseRulesCardSpec| CombatStatCardPlanV1 {
            key: card.key,
            effective_clan_id: card.clan_id,
            ability: CombatStatSourcePlanV1::Absent,
            bonus: CombatStatSourcePlanV1::Absent,
            source_bonus_support_count: 0,
            source_ability_support_count: 0,
        };
        let cards = ByPlayer::new(
            base_rules.players[PlayerId::P1].hand.map(plan),
            base_rules.players[PlayerId::P2].hand.map(plan),
        );
        CombatStatDiagnosticV1::new(CombatStatDiagnosticMatchSpecV1 { base_rules, cards }).unwrap()
    }

    fn commit_zero_round(game: &mut CombatStatDiagnosticV1, slot: u8, first: PlayerId) {
        game.make(BaseRulesRoundInput {
            first_mover: first,
            selections: ByPlayer::new(
                BaseRulesSelection::new(slot, 0, false),
                BaseRulesSelection::new(slot, 0, false),
            ),
        })
        .unwrap();
    }

    fn late_game(life: u16, pillz: u16, p1: Stats, p2: Stats) -> CombatStatDiagnosticV1 {
        let mut game = test_game(life, pillz, p1, p2);
        commit_zero_round(&mut game, 0, PlayerId::P1);
        commit_zero_round(&mut game, 1, PlayerId::P2);
        game
    }

    fn complete(game: &mut CombatStatDiagnosticV1, us: PlayerId, first: PlayerId) -> ExactValue {
        continuation_value(
            game,
            us,
            first,
            &mut PolicyControl::until(Instant::now() + Duration::from_secs(60)),
        )
        .unwrap()
    }

    #[test]
    fn wager_iterator_matches_reference_order_and_counts() {
        assert_eq!(
            Wagers::new(2).collect::<Vec<_>>(),
            vec![
                Wager {
                    pillz: 2,
                    fury: false
                },
                Wager {
                    pillz: 0,
                    fury: false
                },
                Wager {
                    pillz: 1,
                    fury: false
                },
            ]
        );
        let three = Wagers::new(3).collect::<Vec<_>>();
        assert_eq!(three.len(), 5);
        assert_eq!(
            three[0],
            Wager {
                pillz: 3,
                fury: false
            }
        );
        assert_eq!(
            three[1],
            Wager {
                pillz: 0,
                fury: true
            }
        );
        let twelve = Wagers::new(12).collect::<Vec<_>>();
        assert_eq!(twelve.len(), 23);
        assert_eq!(
            twelve[0],
            Wager {
                pillz: 12,
                fury: false
            }
        );
        assert_eq!(
            twelve[1],
            Wager {
                pillz: 9,
                fury: true
            }
        );
        assert!(twelve
            .iter()
            .all(|wager| { wager.pillz + if wager.fury { FURY_COST } else { 0 } <= 12 }));
    }

    #[test]
    fn an_unrepresentable_budget_becomes_an_unbounded_deadline() {
        let control = PolicyControl::for_budget(Instant::now(), Duration::MAX);
        assert_eq!(control.deadline, None);
        assert_eq!(control.nodes(), 0);
    }

    #[test]
    fn both_mover_branches_restore_the_root_and_use_explicit_context() {
        let stats = [(3, 0), (3, 0), (5, 0), (5, 1)];
        let mut game = late_game(1, 0, stats, stats);
        // Leave only the last equal-stat card so the explicit first mover wins its tie.
        commit_zero_round(&mut game, 2, PlayerId::P1);
        let before = game.clone();

        assert_eq!(
            complete(&mut game, PlayerId::P1, PlayerId::P1),
            ExactValue::Win
        );
        assert_eq!(game, before);
        assert_eq!(
            complete(&mut game, PlayerId::P1, PlayerId::P2),
            ExactValue::Loss
        );
        assert_eq!(game, before);
    }

    #[test]
    fn nested_cancellation_unmakes_before_returning_none() {
        let stats = [(3, 0), (3, 0), (5, 1), (5, 1)];
        let mut game = late_game(10, 1, stats, stats);
        let before = game.clone();
        // The first round is made; cancellation is observed before its child round.
        let mut control = PolicyControl::for_nodes(1);
        assert_eq!(
            continuation_value(&mut game, PlayerId::P1, PlayerId::P1, &mut control),
            None
        );
        assert_eq!(control.nodes(), 1);
        assert_eq!(game, before);

        let mut expired = PolicyControl::until(Instant::now());
        assert_eq!(
            continuation_value(&mut game, PlayerId::P1, PlayerId::P2, &mut expired),
            None
        );
        assert_eq!(expired.nodes(), 0);
        assert_eq!(game, before);
    }

    /// Perfect-information reference used only to prove that the production recurrence
    /// does not select a different response after seeing each hidden wager.
    fn peek_value(game: &mut CombatStatDiagnosticV1, us: PlayerId, first: PlayerId) -> ExactValue {
        if let Some(value) = terminal_value(game, us) {
            return value;
        }
        let opponent = us.other();
        if first == us {
            let mut best = ExactValue::Loss;
            for ours in legal_actions(game, us) {
                let mut worst = ExactValue::Win;
                for theirs in legal_actions(game, opponent) {
                    let selections = match us {
                        PlayerId::P1 => ByPlayer::new(ours, theirs),
                        PlayerId::P2 => ByPlayer::new(theirs, ours),
                    };
                    let (_, undo) = game
                        .make(BaseRulesRoundInput {
                            first_mover: first,
                            selections,
                        })
                        .unwrap();
                    let value = peek_value(game, us, first.other());
                    game.unmake(undo);
                    worst = worst.min(value);
                }
                best = best.max(worst);
            }
            best
        } else {
            // The deliberate information leak: choose a fresh response after observing the
            // opponent's complete card/pillz/Fury action.
            let mut worst = ExactValue::Win;
            for theirs in legal_actions(game, opponent) {
                let mut best = ExactValue::Loss;
                for ours in legal_actions(game, us) {
                    let selections = match us {
                        PlayerId::P1 => ByPlayer::new(ours, theirs),
                        PlayerId::P2 => ByPlayer::new(theirs, ours),
                    };
                    let (_, undo) = game
                        .make(BaseRulesRoundInput {
                            first_mover: first,
                            selections,
                        })
                        .unwrap();
                    let value = peek_value(game, us, first.other());
                    game.unmake(undo);
                    best = best.max(value);
                }
                worst = worst.min(best);
            }
            worst
        }
    }

    #[test]
    fn one_response_must_cover_every_hidden_wager_for_a_visible_card() {
        let mut game = late_game(
            2,
            1,
            [(1, 0), (1, 0), (2, 1), (4, 1)],
            [(1, 0), (1, 0), (1, 1), (3, 2)],
        );
        let before = game.clone();

        assert_eq!(
            complete(&mut game, PlayerId::P1, PlayerId::P2),
            ExactValue::Draw
        );
        assert_eq!(game, before);
        assert_eq!(
            peek_value(&mut game, PlayerId::P1, PlayerId::P2),
            ExactValue::Win
        );
        assert_eq!(game, before);
    }

    /// More conservative test-only policy that wrongly requires one response across every
    /// opponent card as well as every wager.
    fn blind_fixed_response(
        game: &mut CombatStatDiagnosticV1,
        us: PlayerId,
        first: PlayerId,
    ) -> ExactValue {
        let opponent = us.other();
        let mut best = ExactValue::Loss;
        for ours in legal_actions(game, us) {
            let mut worst = ExactValue::Win;
            for theirs in legal_actions(game, opponent) {
                let selections = match us {
                    PlayerId::P1 => ByPlayer::new(ours, theirs),
                    PlayerId::P2 => ByPlayer::new(theirs, ours),
                };
                let (_, undo) = game
                    .make(BaseRulesRoundInput {
                        first_mover: first,
                        selections,
                    })
                    .unwrap();
                let value = complete(game, us, first.other());
                game.unmake(undo);
                worst = worst.min(value);
            }
            best = best.max(worst);
        }
        best
    }

    #[test]
    fn response_may_change_when_the_visible_opponent_card_changes() {
        let mut game = late_game(
            1,
            0,
            [(1, 0), (1, 0), (1, 0), (2, 1)],
            [(1, 0), (1, 0), (1, 0), (3, 0)],
        );
        let before = game.clone();

        assert_eq!(
            complete(&mut game, PlayerId::P1, PlayerId::P2),
            ExactValue::Win
        );
        assert_eq!(game, before);
        assert_eq!(
            blind_fixed_response(&mut game, PlayerId::P1, PlayerId::P2),
            ExactValue::Draw
        );
        assert_eq!(game, before);
    }
}
