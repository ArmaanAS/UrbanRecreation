//! Responsive current-round search for the replay-grounded advisor.
//!
//! Every current-round pairing is resolved by the real [`CombatStatDiagnosticV1`] engine.
//! Round one folds in a bounded opening heuristic; rounds two through four use the exact
//! information-aware continuation in `policy`. The split is explicit so the UI never
//! presents an opening estimate as a solved future game.

use std::cmp::Ordering;
use std::time::{Duration, Instant};

use crate::engine::{
    BaseRulesRoundInput, BaseRulesSelection, ByPlayer, CombatStatDiagnosticV1, MatchStatus,
    PlayerId, FURY_COST,
};

use super::policy::{continuation_value, ExactValue, PolicyControl};

/// Semantic identity of the live recommendation policy, including the fixed historical
/// opening prior below. Bump this whenever ranking, continuation, or opening-weight
/// semantics change in a way that can change a recommendation.
pub const ADVISOR_POLICY_SEMANTIC_REVISION_V1: u16 = 1;

/// A wager in engine notation. `pillz` excludes the free attack pill and the Fury cost.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AdvisorMove {
    pub hand_index: u8,
    pub pillz: u16,
    pub fury: bool,
}

impl AdvisorMove {
    const fn cost(self) -> u16 {
        self.pillz + if self.fury { FURY_COST } else { 0 }
    }

    const fn as_selection(self) -> BaseRulesSelection {
        BaseRulesSelection::new(self.hand_index, self.pillz, self.fury)
    }
}

/// Which current-round uncertainty the root matrix represents.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SearchMode {
    /// We move first. Each recommendation is sampled against every opposing response.
    First,
    /// The opponent's card is visible, but its pillz and Fury remain hidden.
    Second { opponent_hand_index: u8 },
    /// The opponent moves first but has not revealed a card. Every row is one fixed reply
    /// against every unplayed opponent card and its hidden pillz/Fury wager.
    BlindSecond,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SearchConfig {
    pub us: PlayerId,
    pub first_mover: PlayerId,
    pub mode: SearchMode,
    pub budget: Duration,
}

/// How nonterminal current-round samples are evaluated.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EvaluationKind {
    /// Captured-reply-weighted position estimate used for the opening round only.
    OpeningEstimate,
    /// Exact conservative continuation policy for roots in rounds two through four.
    ExactContinuationPolicy,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RankedMove {
    pub move_: AdvisorMove,
    /// Mean in our frame: captured-reply weighted in the opening, uniform afterwards.
    pub average: f64,
    /// Lowest complete opposing sample seen for this move, also in our frame.
    /// This is a game-theoretic Worst only for exact-continuation rows.
    pub worst: f64,
    /// Highest complete opposing sample seen for this move, also in our frame.
    /// Together with `worst`, opening rows show the observed floor-to-ceiling Range.
    pub best: f64,
    /// Complete opposing samples folded into this result.
    pub samples: usize,
    pub kos: usize,
    pub koed: usize,
    /// Per-hidden-wager values are retained only for a precise second-mover root.  The
    /// JSONL boundary publishes them only in a complete final response; progress and the
    /// other information sets deliberately expose no speculative per-wager panel.
    pub hidden_outcomes: Vec<HiddenOutcome>,
}

/// One precise second-mover result for a hidden opponent wager, in the requester's frame.
/// `flags` uses bit 1 for an immediate KO and bit 2 for being KO'd immediately.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HiddenOutcome {
    pub opponent_pillz: u16,
    pub opponent_fury: bool,
    pub value: f64,
    pub flags: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchSnapshot {
    pub ranked: Vec<RankedMove>,
    pub units_done: usize,
    pub units_total: usize,
    pub elapsed: Duration,
    pub complete: bool,
    pub evaluation: EvaluationKind,
}

#[derive(Clone, Copy, Debug)]
struct Sample {
    value: f64,
    ko: bool,
    koed: bool,
}

#[derive(Clone, Debug)]
struct Candidate {
    move_: AdvisorMove,
    weighted_sum: f64,
    total_weight: u32,
    worst: f64,
    best: f64,
    samples: usize,
    kos: usize,
    koed: usize,
    hidden_outcomes: Vec<HiddenOutcome>,
}

impl Candidate {
    fn new(move_: AdvisorMove) -> Self {
        Self {
            move_,
            weighted_sum: 0.0,
            total_weight: 0,
            worst: f64::NAN,
            best: f64::NAN,
            samples: 0,
            kos: 0,
            koed: 0,
            hidden_outcomes: Vec::new(),
        }
    }

    fn push(&mut self, sample: Sample, weight: u16) {
        self.weighted_sum += sample.value * f64::from(weight);
        self.total_weight += u32::from(weight);
        self.worst = if self.samples == 0 {
            sample.value
        } else {
            self.worst.min(sample.value)
        };
        self.best = if self.samples == 0 {
            sample.value
        } else {
            self.best.max(sample.value)
        };
        self.samples += 1;
        self.kos += usize::from(sample.ko);
        self.koed += usize::from(sample.koed);
    }

    fn ranked(&self) -> RankedMove {
        RankedMove {
            move_: self.move_,
            average: if self.samples == 0 {
                f64::NAN
            } else {
                self.weighted_sum / f64::from(self.total_weight)
            },
            worst: self.worst,
            best: self.best,
            samples: self.samples,
            kos: self.kos,
            koed: self.koed,
            hidden_outcomes: self.hidden_outcomes.clone(),
        }
    }

    fn push_hidden_outcome(&mut self, opponent_move: AdvisorMove, sample: Sample) {
        self.hidden_outcomes.push(HiddenOutcome {
            opponent_pillz: opponent_move.pillz,
            opponent_fury: opponent_move.fury,
            value: sample.value,
            flags: u8::from(sample.ko) | (u8::from(sample.koed) << 1),
        });
    }
}

// 198 captured round-one plays as of 2026-09-13. This is intentionally a literal
// historical table, copied from the TypeScript advisor rather than regenerated from the
// current capture corpus. Add one Laplace observation to every legal reply so an unseen
// wager remains possible.
const OPENING_REPLY_COUNTS: &[((u16, bool), u16)] = &[
    ((0, false), 38),
    ((1, false), 7),
    ((2, false), 20),
    ((3, false), 28),
    ((4, false), 53),
    ((4, true), 1),
    ((5, false), 29),
    ((6, false), 12),
    ((7, false), 7),
    ((8, false), 1),
    ((9, true), 2),
];

fn opening_reply_weight(move_: AdvisorMove) -> u16 {
    OPENING_REPLY_COUNTS
        .iter()
        .find_map(|&((pillz, fury), count)| {
            (pillz == move_.pillz && fury == move_.fury).then_some(count)
        })
        .unwrap_or(0)
        + 1
}

/// Enumerates every legal card/wager action in the same useful probe order as the
/// TypeScript search: plain all-in first, then the largest Fury wager, cheap probes, and
/// finally the two near-all-in plain wagers. Hand slots remain in ascending order.
pub fn legal_moves(game: &CombatStatDiagnosticV1, player: PlayerId) -> Vec<AdvisorMove> {
    if game.position().status != MatchStatus::Playing {
        return Vec::new();
    }

    let available = game.position().players[player].pillz;
    let bets = ordered_bets(available);
    let mut moves = Vec::new();
    for (hand_index, played) in game.position().played[player].iter().copied().enumerate() {
        if played {
            continue;
        }
        for &(pillz, fury) in &bets {
            moves.push(AdvisorMove {
                hand_index: hand_index as u8,
                pillz,
                fury,
            });
        }
    }
    moves
}

fn ordered_bets(available: u16) -> Vec<(u16, bool)> {
    let mut paid = Vec::with_capacity(usize::from(available) + 1);
    paid.push(available);
    if available < FURY_COST {
        paid.extend(0..available);
    } else {
        paid.push(available - FURY_COST);
        paid.extend(0..available - FURY_COST);
        paid.extend(available - 2..available);
    }

    let mut bets = Vec::with_capacity(if available < FURY_COST {
        usize::from(available) + 1
    } else {
        usize::from(available) * 2 - 1
    });
    for pillz in paid {
        if pillz <= available.saturating_sub(FURY_COST) && available >= FURY_COST {
            bets.push((pillz, true));
        }
        bets.push((pillz, false));
    }
    bets
}

/// Evaluates the current round until every pairing is complete or the time budget expires.
///
/// The callback receives immutable, already-ranked snapshots at bounded useful boundaries:
/// after a full reply set in First mode or one complete unknown-opponent column across all
/// replies in Second and BlindSecond modes.
/// Publication happens only after undo restores the root, so cancellation and callback
/// code can never observe a half-resolved position.
pub fn search(
    game: &mut CombatStatDiagnosticV1,
    config: SearchConfig,
    progress: impl FnMut(&SearchSnapshot),
) -> SearchSnapshot {
    let started = Instant::now();
    let mut policy_control = PolicyControl::for_budget(started, config.budget);
    search_with_control(game, config, started, &mut policy_control, progress)
}

#[cfg(test)]
fn search_with_test_control(
    game: &mut CombatStatDiagnosticV1,
    config: SearchConfig,
    policy_control: &mut PolicyControl,
    progress: impl FnMut(&SearchSnapshot),
) -> SearchSnapshot {
    search_with_control(game, config, Instant::now(), policy_control, progress)
}

fn search_with_control(
    game: &mut CombatStatDiagnosticV1,
    config: SearchConfig,
    started: Instant,
    policy_control: &mut PolicyControl,
    mut progress: impl FnMut(&SearchSnapshot),
) -> SearchSnapshot {
    let evaluation = if game.position().rounds_played >= 1 {
        EvaluationKind::ExactContinuationPolicy
    } else {
        EvaluationKind::OpeningEstimate
    };
    let opponent = config.us.other();
    let our_moves = legal_moves(game, config.us);
    let opponent_moves = match config.mode {
        SearchMode::First => legal_moves(game, opponent),
        SearchMode::Second {
            opponent_hand_index,
        } => legal_moves(game, opponent)
            .into_iter()
            .filter(|move_| move_.hand_index == opponent_hand_index)
            .collect(),
        // The opponent has not committed a visible card yet. Keep every card/wager action
        // as a separate hypothesis, while candidates below remain our fixed replies.
        SearchMode::BlindSecond => legal_moves(game, opponent),
    };
    let units_total = our_moves.len().saturating_mul(opponent_moves.len());
    let mut candidates: Vec<_> = our_moves.into_iter().map(Candidate::new).collect();
    let mut units_done = 0;
    let mut expired = false;
    let mut last_publication: Option<(usize, bool)> = None;

    // A publication is transactional. First mode commits a whole reply row; Second and
    // BlindSecond modes commit one whole hidden-opponent column. A deadline or policy
    // cancellation inside either block discards its buffered samples, so ranked candidates
    // never contain incomparable partial blocks and `units_done` always counts published
    // work.
    match config.mode {
        SearchMode::First => {
            'matrix: for candidate_index in 0..candidates.len() {
                let mut row = Vec::with_capacity(opponent_moves.len());
                for &opponent_move in &opponent_moves {
                    if started.elapsed() >= config.budget {
                        expired = true;
                        break 'matrix;
                    }
                    let Some(sample) = evaluate_pair(
                        game,
                        config,
                        candidates[candidate_index].move_,
                        opponent_move,
                        evaluation,
                        policy_control,
                    ) else {
                        expired = true;
                        break 'matrix;
                    };
                    row.push((sample, sample_weight(evaluation, opponent_move)));
                }
                for (sample, weight) in row {
                    candidates[candidate_index].push(sample, weight);
                }
                units_done += opponent_moves.len();
                // This recommendation is now comparable across its complete reply set.
                publish(
                    &candidates,
                    units_done,
                    units_total,
                    started.elapsed(),
                    evaluation,
                    &mut progress,
                    &mut last_publication,
                );
            }
        }
        SearchMode::Second { .. } | SearchMode::BlindSecond => {
            let retain_hidden_outcomes = matches!(config.mode, SearchMode::Second { .. });
            'matrix: for &opponent_move in &opponent_moves {
                let mut column = Vec::with_capacity(candidates.len());
                for candidate in &candidates {
                    if started.elapsed() >= config.budget {
                        expired = true;
                        break 'matrix;
                    }
                    let Some(sample) = evaluate_pair(
                        game,
                        config,
                        candidate.move_,
                        opponent_move,
                        evaluation,
                        policy_control,
                    ) else {
                        expired = true;
                        break 'matrix;
                    };
                    column.push((sample, sample_weight(evaluation, opponent_move)));
                }
                for (candidate, (sample, weight)) in candidates.iter_mut().zip(column) {
                    candidate.push(sample, weight);
                    if retain_hidden_outcomes {
                        candidate.push_hidden_outcome(opponent_move, sample);
                    }
                }
                units_done += candidates.len();
                // This exact hidden wager (and, in blind mode, its card) has now been
                // tested against every possible fixed response.
                publish(
                    &candidates,
                    units_done,
                    units_total,
                    started.elapsed(),
                    evaluation,
                    &mut progress,
                    &mut last_publication,
                );
            }
        }
    }

    let complete = !expired && units_done == units_total;
    let result = snapshot(
        &candidates,
        units_done,
        units_total,
        started.elapsed(),
        complete,
        evaluation,
    );
    // Always send a final state for a zero-budget, empty, or between-boundaries stop, but
    // avoid cloning and repainting the final complete matrix twice.
    if last_publication != Some((units_done, complete)) {
        progress(&result);
    }
    result
}

fn evaluate_pair(
    game: &mut CombatStatDiagnosticV1,
    config: SearchConfig,
    our_move: AdvisorMove,
    opponent_move: AdvisorMove,
    evaluation: EvaluationKind,
    policy_control: &mut PolicyControl,
) -> Option<Sample> {
    let input = round_input(config.first_mover, config.us, our_move, opponent_move);
    let (_, undo) = game
        .make(input)
        .expect("legal advisor moves must execute in a fully admitted match");
    let sample = evaluate(
        game,
        config.us,
        evaluation,
        config.first_mover.other(),
        policy_control,
    );
    game.unmake(undo);
    sample
}

fn publish(
    candidates: &[Candidate],
    units_done: usize,
    units_total: usize,
    elapsed: Duration,
    evaluation: EvaluationKind,
    progress: &mut impl FnMut(&SearchSnapshot),
    last_publication: &mut Option<(usize, bool)>,
) {
    let complete = units_done == units_total;
    let update = snapshot(
        candidates,
        units_done,
        units_total,
        elapsed,
        complete,
        evaluation,
    );
    progress(&update);
    *last_publication = Some((units_done, complete));
}

fn round_input(
    first_mover: PlayerId,
    us: PlayerId,
    our_move: AdvisorMove,
    opponent_move: AdvisorMove,
) -> BaseRulesRoundInput {
    let selections = match us {
        PlayerId::P1 => ByPlayer::new(our_move.as_selection(), opponent_move.as_selection()),
        PlayerId::P2 => ByPlayer::new(opponent_move.as_selection(), our_move.as_selection()),
    };
    BaseRulesRoundInput {
        first_mover,
        selections,
    }
}

fn evaluate(
    game: &mut CombatStatDiagnosticV1,
    us: PlayerId,
    evaluation: EvaluationKind,
    next_first_mover: PlayerId,
    policy_control: &mut PolicyControl,
) -> Option<Sample> {
    let opponent = us.other();
    let status = game.position().status;
    let opponent_life = game.position().players[opponent].life;
    let our_life = game.position().players[us].life;
    let value = match status {
        MatchStatus::Won(winner) if winner == us => 1.0,
        MatchStatus::Won(_) => -1.0,
        MatchStatus::Draw => 0.0,
        MatchStatus::Playing => match evaluation {
            EvaluationKind::OpeningEstimate => position_heuristic(game, us),
            EvaluationKind::ExactContinuationPolicy => exact_score(continuation_value(
                game,
                us,
                next_first_mover,
                policy_control,
            )?),
        },
    };
    Some(Sample {
        value,
        ko: value == 1.0 && opponent_life == 0,
        // A double knockout still counts as being knocked out, matching the TypeScript UI.
        koed: our_life == 0,
    })
}

const fn exact_score(value: ExactValue) -> f64 {
    match value {
        ExactValue::Loss => -1.0,
        ExactValue::Draw => 0.0,
        ExactValue::Win => 1.0,
    }
}

fn sample_weight(evaluation: EvaluationKind, opponent_move: AdvisorMove) -> u16 {
    match evaluation {
        EvaluationKind::OpeningEstimate => opening_reply_weight(opponent_move),
        EvaluationKind::ExactContinuationPolicy => 1,
    }
}

/// A deliberately bounded one-round estimate in the asking player's frame.
///
/// Life is the primary term, pillz retain nonlinear reserve value, and the base power plus
/// damage of unplayed cards only breaks otherwise close positions. Future conditional
/// effects are not projected here; `tanh` keeps the public result strictly inside [-1, 1].
fn position_heuristic(game: &CombatStatDiagnosticV1, us: PlayerId) -> f64 {
    let opponent = us.other();
    let position = game.position();
    let spec = game.base_rules_spec();
    let remaining = |player: PlayerId| -> f64 {
        spec.players[player]
            .hand
            .iter()
            .enumerate()
            .filter(|(index, _)| !position.played[player][*index])
            .map(|(_, card)| f64::from(card.power) + f64::from(card.damage))
            .sum()
    };
    let life = f64::from(position.players[us].life) - f64::from(position.players[opponent].life);
    let pillz = 4.0
        * (f64::from(position.players[us].pillz).sqrt()
            - f64::from(position.players[opponent].pillz).sqrt());
    let cards = remaining(us) - remaining(opponent);
    ((life + pillz + 0.12 * cards) / 12.0).tanh()
}

fn snapshot(
    candidates: &[Candidate],
    units_done: usize,
    units_total: usize,
    elapsed: Duration,
    complete: bool,
    evaluation: EvaluationKind,
) -> SearchSnapshot {
    let mut ranked: Vec<_> = candidates.iter().map(Candidate::ranked).collect();
    ranked.sort_by(|left, right| compare_ranked(left, right, evaluation));
    SearchSnapshot {
        ranked,
        units_done,
        units_total,
        elapsed,
        complete,
        evaluation,
    }
}

fn displayed_percent(value: f64) -> i32 {
    if value.is_nan() {
        return i32::MIN;
    }
    let percent = ((value + 1.0) * 50.0).round() as i32;
    if percent >= 100 && value < 1.0 {
        99
    } else if percent <= 0 && value > -1.0 {
        1
    } else {
        percent
    }
}

fn compare_ranked(left: &RankedMove, right: &RankedMove, evaluation: EvaluationKind) -> Ordering {
    // Higher displayed average first. Deliberately ignore sub-percent raw differences:
    // equal-looking rows should be ordered by the visible tie-breaks. Exact policy rows
    // then use their game-theoretic Worst; opening rows expose a descriptive Range, never
    // a guarantee, so deliberately skip that floor.
    let average = displayed_percent(right.average).cmp(&displayed_percent(left.average));
    let worst = if evaluation == EvaluationKind::ExactContinuationPolicy {
        displayed_percent(right.worst).cmp(&displayed_percent(left.worst))
    } else {
        Ordering::Equal
    };
    average
        .then(worst)
        .then_with(|| share(right.kos, right.samples).total_cmp(&share(left.kos, left.samples)))
        .then_with(|| share(left.koed, left.samples).total_cmp(&share(right.koed, right.samples)))
        .then_with(|| left.move_.cost().cmp(&right.move_.cost()))
        .then_with(|| left.move_.hand_index.cmp(&right.move_.hand_index))
        .then_with(|| left.move_.fury.cmp(&right.move_.fury))
        .then_with(|| left.move_.pillz.cmp(&right.move_.pillz))
}

fn share(count: usize, samples: usize) -> f64 {
    if samples == 0 {
        0.0
    } else {
        count as f64 / samples as f64
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

    fn card(id: u32, power: u16, damage: u16) -> BaseRulesCardSpec {
        BaseRulesCardSpec {
            key: CardKey::new(id, 1),
            clan_id: id,
            power,
            damage,
        }
    }

    fn test_game(life: u16, pillz: u16, p1: (u16, u16), p2: (u16, u16)) -> CombatStatDiagnosticV1 {
        let hand = |base: u32, stats: (u16, u16)| {
            std::array::from_fn(|index| card(base + index as u32, stats.0, stats.1))
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

    fn config(mode: SearchMode, budget: Duration) -> SearchConfig {
        SearchConfig {
            us: PlayerId::P1,
            first_mover: PlayerId::P1,
            mode,
            budget,
        }
    }

    fn blind_second_config(budget: Duration) -> SearchConfig {
        SearchConfig {
            us: PlayerId::P1,
            first_mover: PlayerId::P2,
            mode: SearchMode::BlindSecond,
            budget,
        }
    }

    fn shallow_evaluate(game: &mut CombatStatDiagnosticV1, us: PlayerId) -> Sample {
        let mut control = PolicyControl::for_budget(Instant::now(), Duration::from_secs(1));
        evaluate(
            game,
            us,
            EvaluationKind::OpeningEstimate,
            PlayerId::P1,
            &mut control,
        )
        .unwrap()
    }

    #[test]
    fn legal_actions_match_the_reference_counts_and_fury_cost() {
        let twelve = test_game(20, 12, (6, 3), (6, 3));
        let moves = legal_moves(&twelve, PlayerId::P1);
        assert_eq!(moves.len(), 4 * 23);
        assert_eq!(
            moves[0],
            AdvisorMove {
                hand_index: 0,
                pillz: 12,
                fury: false
            }
        );
        assert_eq!(
            moves[1],
            AdvisorMove {
                hand_index: 0,
                pillz: 9,
                fury: true
            }
        );
        assert!(moves.iter().all(|move_| move_.cost() <= 12));
        assert_eq!(moves.iter().filter(|move_| move_.fury).count(), 4 * 10);

        let two = test_game(20, 2, (6, 3), (6, 3));
        let moves = legal_moves(&two, PlayerId::P1);
        assert_eq!(moves.len(), 4 * 3);
        assert!(moves.iter().all(|move_| !move_.fury));
    }

    #[test]
    fn opening_reply_table_is_literal_and_laplace_smooths_unknown_wagers() {
        assert_eq!(
            OPENING_REPLY_COUNTS
                .iter()
                .map(|(_, count)| u32::from(*count))
                .sum::<u32>(),
            198,
        );
        assert_eq!(
            opening_reply_weight(AdvisorMove {
                hand_index: 0,
                pillz: 4,
                fury: false,
            }),
            54,
        );
        assert_eq!(
            opening_reply_weight(AdvisorMove {
                hand_index: 0,
                pillz: 9,
                fury: true,
            }),
            3,
        );
        assert_eq!(
            opening_reply_weight(AdvisorMove {
                hand_index: 0,
                pillz: 12,
                fury: false,
            }),
            1,
        );
    }

    #[test]
    fn opening_weighted_mean_preserves_unweighted_samples_and_range() {
        let mut candidate = Candidate::new(AdvisorMove {
            hand_index: 0,
            pillz: 0,
            fury: false,
        });
        candidate.push(
            Sample {
                value: -1.0,
                ko: true,
                koed: false,
            },
            54,
        );
        candidate.push(
            Sample {
                value: 1.0,
                ko: false,
                koed: true,
            },
            1,
        );
        let ranked = candidate.ranked();
        assert_eq!(ranked.average, -53.0 / 55.0);
        assert_eq!((ranked.worst, ranked.best), (-1.0, 1.0));
        assert_eq!((ranked.samples, ranked.kos, ranked.koed), (2, 1, 1));
    }

    #[test]
    fn zero_budget_and_complete_search_both_preserve_the_root() {
        let mut game = test_game(20, 0, (7, 3), (6, 2));
        let before = game.clone();
        let mut callbacks = 0;
        let stopped = search(&mut game, config(SearchMode::First, Duration::ZERO), |_| {
            callbacks += 1
        });
        assert!(!stopped.complete);
        assert_eq!(stopped.units_done, 0);
        assert_eq!(callbacks, 1);
        assert_eq!(game, before);

        let completed = search(
            &mut game,
            config(SearchMode::First, Duration::from_secs(1)),
            |_| {},
        );
        assert!(completed.complete);
        assert_eq!((completed.units_done, completed.units_total), (16, 16));
        assert_eq!(game, before);
    }

    #[test]
    fn deadline_does_not_materialize_an_unusually_large_pair_matrix() {
        let mut game = test_game(20, 5_000, (7, 3), (6, 2));
        let before = game.clone();
        let stopped = search(&mut game, config(SearchMode::First, Duration::ZERO), |_| {});
        assert_eq!(stopped.units_done, 0);
        assert!(stopped.units_total > 1_000_000_000);
        assert!(!stopped.complete);
        assert_eq!(game, before);
    }

    #[test]
    fn first_and_second_modes_build_the_right_response_sets() {
        // Three pillz gives each card five actions: 3 plain, 0 Fury, 0/1/2 plain.
        let mut game = test_game(20, 3, (7, 3), (6, 2));
        let mut first_updates = 0;
        let first = search(
            &mut game,
            config(SearchMode::First, Duration::from_secs(1)),
            |_| first_updates += 1,
        );
        assert_eq!(first.ranked.len(), 20);
        assert_eq!(first.units_total, 400);
        assert_eq!(first_updates, 20);
        assert_eq!(first.evaluation, EvaluationKind::OpeningEstimate);
        assert!(first.ranked.iter().all(|candidate| candidate.samples == 20));
        assert!(first
            .ranked
            .iter()
            .all(|candidate| candidate.hidden_outcomes.is_empty()));

        let mut second_updates = 0;
        let second = search(
            &mut game,
            config(
                SearchMode::Second {
                    opponent_hand_index: 2,
                },
                Duration::from_secs(1),
            ),
            |_| second_updates += 1,
        );
        assert_eq!(second.ranked.len(), 20);
        assert_eq!(second.units_total, 100);
        assert_eq!(second_updates, 5);
        assert!(second.ranked.iter().all(|candidate| candidate.samples == 5));
        assert!(second.ranked.iter().all(|candidate| {
            candidate.hidden_outcomes.len() == 5
                && candidate.hidden_outcomes.iter().all(|outcome| {
                    outcome.opponent_pillz <= 3 && outcome.flags <= 3 && outcome.value.is_finite()
                })
        }));
    }

    #[test]
    fn blind_second_groups_every_opponent_card_and_wager_into_fixed_reply_rows() {
        // Round three leaves two cards each. With one pill, each card has two plain
        // actions, so blind mode has four opponent card/wager hypotheses and four fixed
        // replies. The completed columns make every row directly comparable.
        let mut game = round_three_game(1);
        let before = game.clone();
        let mut updates = 0;
        let result = search(
            &mut game,
            blind_second_config(Duration::from_secs(1)),
            |_| updates += 1,
        );

        assert!(result.complete);
        assert_eq!(result.evaluation, EvaluationKind::ExactContinuationPolicy);
        assert_eq!(
            (result.ranked.len(), result.units_done, result.units_total),
            (4, 16, 16)
        );
        assert_eq!(updates, 4);
        assert!(result.ranked.iter().all(|candidate| candidate.samples == 4));
        assert!(result
            .ranked
            .iter()
            .all(|candidate| candidate.hidden_outcomes.is_empty()));
        assert_eq!(game, before);
    }

    #[test]
    fn blind_second_uses_exact_continuations_from_round_two() {
        let mut game = test_game(20, 0, (7, 3), (6, 2));
        game.make(round_input(
            PlayerId::P1,
            PlayerId::P1,
            AdvisorMove {
                hand_index: 0,
                pillz: 0,
                fury: false,
            },
            AdvisorMove {
                hand_index: 0,
                pillz: 0,
                fury: false,
            },
        ))
        .unwrap();
        let before = game.clone();
        let result = search(
            &mut game,
            blind_second_config(Duration::from_secs(1)),
            |_| {},
        );

        assert!(result.complete);
        assert_eq!(result.evaluation, EvaluationKind::ExactContinuationPolicy);
        assert_eq!(
            (result.ranked.len(), result.units_done, result.units_total),
            (3, 9, 9)
        );
        assert!(result.ranked.iter().all(|candidate| {
            (candidate.average, candidate.worst, candidate.best) == (1.0, 1.0, 1.0)
        }));
        assert_eq!(game, before);
    }

    #[test]
    fn opening_search_weights_opponent_wagers_in_both_modes_and_keeps_a_fixed_top() {
        // With one pill, the observed 0-pill reply (weight 39) dominates the observed
        // 1-pill reply (weight 8). The stronger P1 cards make saving the pill the stable
        // opening recommendation in either visible-information mode.
        let mut game = test_game(20, 1, (8, 4), (5, 2));
        let expected_top = AdvisorMove {
            hand_index: 0,
            pillz: 0,
            fury: false,
        };
        let first = search(
            &mut game,
            config(SearchMode::First, Duration::from_secs(1)),
            |_| {},
        );
        let second = search(
            &mut game,
            config(
                SearchMode::Second {
                    opponent_hand_index: 0,
                },
                Duration::from_secs(1),
            ),
            |_| {},
        );
        assert_eq!(first.evaluation, EvaluationKind::OpeningEstimate);
        assert_eq!(second.evaluation, EvaluationKind::OpeningEstimate);
        assert_eq!(first.ranked[0].move_, expected_top);
        assert_eq!(second.ranked[0].move_, expected_top);
        let first_top = first
            .ranked
            .iter()
            .find(|candidate| candidate.move_ == expected_top)
            .unwrap();
        let second_top = second
            .ranked
            .iter()
            .find(|candidate| candidate.move_ == expected_top)
            .unwrap();
        assert!(first_top.average > 0.4 && second_top.average > 0.4);
        assert_eq!((first_top.samples, second_top.samples), (8, 2));
    }

    #[test]
    fn evaluation_uses_exact_terminal_values_and_a_symmetric_opening_estimate() {
        let mut knockout = test_game(1, 0, (10, 2), (1, 1));
        let (_, undo) = knockout
            .make(round_input(
                PlayerId::P1,
                PlayerId::P1,
                AdvisorMove {
                    hand_index: 0,
                    pillz: 0,
                    fury: false,
                },
                AdvisorMove {
                    hand_index: 0,
                    pillz: 0,
                    fury: false,
                },
            ))
            .unwrap();
        let ours = shallow_evaluate(&mut knockout, PlayerId::P1);
        let theirs = shallow_evaluate(&mut knockout, PlayerId::P2);
        assert_eq!((ours.value, ours.ko, ours.koed), (1.0, true, false));
        assert_eq!((theirs.value, theirs.ko, theirs.koed), (-1.0, false, true));
        knockout.unmake(undo);

        let mut position = test_game(20, 3, (8, 4), (5, 2));
        let (_, undo) = position
            .make(round_input(
                PlayerId::P1,
                PlayerId::P1,
                AdvisorMove {
                    hand_index: 0,
                    pillz: 1,
                    fury: false,
                },
                AdvisorMove {
                    hand_index: 0,
                    pillz: 0,
                    fury: false,
                },
            ))
            .unwrap();
        let p1 = shallow_evaluate(&mut position, PlayerId::P1).value;
        let p2 = shallow_evaluate(&mut position, PlayerId::P2).value;
        assert!(p1.abs() < 1.0);
        assert_eq!(p1, -p2);
        position.unmake(undo);
    }

    #[test]
    fn round_two_uses_exact_categorical_continuations_and_restores_root() {
        // With no paid pillz and strictly stronger P1 cards, every current-round sample
        // and every exact continuation is a P1 win. This keeps the round-two policy test
        // small while proving the search does not fall back to a fractional heuristic.
        let mut game = test_game(20, 0, (7, 3), (6, 2));
        game.make(round_input(
            PlayerId::P1,
            PlayerId::P1,
            AdvisorMove {
                hand_index: 0,
                pillz: 0,
                fury: false,
            },
            AdvisorMove {
                hand_index: 0,
                pillz: 0,
                fury: false,
            },
        ))
        .unwrap();
        let before = game.clone();
        let result = search(
            &mut game,
            config(SearchMode::First, Duration::from_secs(1)),
            |_| {},
        );
        assert!(result.complete);
        assert_eq!(result.evaluation, EvaluationKind::ExactContinuationPolicy);
        assert_eq!((result.units_done, result.units_total), (9, 9));
        assert!(result.ranked.iter().all(|candidate| {
            (
                candidate.average,
                candidate.worst,
                candidate.best,
                candidate.samples,
            ) == (1.0, 1.0, 1.0, 3)
        }));
        assert_eq!(game, before);
    }

    #[test]
    fn round_two_deadline_cancels_exact_policy_work_and_restores_root() {
        let mut game = test_game(20, 0, (7, 3), (6, 2));
        game.make(round_input(
            PlayerId::P1,
            PlayerId::P1,
            AdvisorMove {
                hand_index: 0,
                pillz: 0,
                fury: false,
            },
            AdvisorMove {
                hand_index: 0,
                pillz: 0,
                fury: false,
            },
        ))
        .unwrap();
        let before = game.clone();
        // An already-expired policy deadline is reached after `evaluate_pair` has made
        // the current round. The pair must still unmake it before propagating `None`.
        let mut expired_policy = PolicyControl::until(Instant::now());
        assert!(evaluate_pair(
            &mut game,
            config(SearchMode::First, Duration::from_secs(1)),
            AdvisorMove {
                hand_index: 1,
                pillz: 0,
                fury: false,
            },
            AdvisorMove {
                hand_index: 1,
                pillz: 0,
                fury: false,
            },
            EvaluationKind::ExactContinuationPolicy,
            &mut expired_policy,
        )
        .is_none(),);
        assert_eq!(game, before);

        let stopped = search(&mut game, config(SearchMode::First, Duration::ZERO), |_| {});
        assert_eq!(stopped.evaluation, EvaluationKind::ExactContinuationPolicy);
        assert!(!stopped.complete);
        assert_eq!(stopped.units_done, 0);
        assert_eq!(game, before);
    }

    #[test]
    fn round_three_uses_exact_information_aware_continuations_and_restores_root() {
        let mut game = test_game(20, 0, (7, 3), (6, 2));
        for (slot, first_mover) in [(0, PlayerId::P1), (1, PlayerId::P2)] {
            game.make(round_input(
                first_mover,
                PlayerId::P1,
                AdvisorMove {
                    hand_index: slot,
                    pillz: 0,
                    fury: false,
                },
                AdvisorMove {
                    hand_index: slot,
                    pillz: 0,
                    fury: false,
                },
            ))
            .unwrap();
        }
        let before = game.clone();
        let result = search(
            &mut game,
            config(SearchMode::First, Duration::from_secs(1)),
            |_| {},
        );
        assert!(result.complete);
        assert_eq!(result.evaluation, EvaluationKind::ExactContinuationPolicy);
        assert_eq!((result.units_done, result.units_total), (4, 4));
        assert!(result.ranked.iter().all(|candidate| {
            (-1.0..=1.0).contains(&candidate.average) && [-1.0, 0.0, 1.0].contains(&candidate.worst)
        }));
        assert_eq!(game, before);
    }

    fn round_three_game(pillz: u16) -> CombatStatDiagnosticV1 {
        let mut game = test_game(20, pillz, (7, 3), (6, 2));
        for (slot, first_mover) in [(0, PlayerId::P1), (1, PlayerId::P2)] {
            game.make(round_input(
                first_mover,
                PlayerId::P1,
                AdvisorMove {
                    hand_index: slot,
                    pillz: 0,
                    fury: false,
                },
                AdvisorMove {
                    hand_index: slot,
                    pillz: 0,
                    fury: false,
                },
            ))
            .unwrap();
        }
        game
    }

    #[test]
    fn first_mode_discards_a_cancelled_reply_row_before_publishing() {
        let mut game = round_three_game(0);
        let before = game.clone();
        // Each completed zero-pill line needs one policy node here. The third node lands
        // in the next row, whose second reply then observes cancellation.
        let mut control = PolicyControl::for_nodes(3);
        let mut callbacks = Vec::new();
        let result = search_with_test_control(
            &mut game,
            config(SearchMode::First, Duration::from_secs(1)),
            &mut control,
            |snapshot| callbacks.push(snapshot.clone()),
        );
        assert_eq!(control.nodes(), 3);
        assert!(!result.complete);
        assert_eq!((result.units_done, result.units_total), (2, 4));
        assert_eq!(
            result
                .ranked
                .iter()
                .map(|candidate| candidate.samples)
                .collect::<Vec<_>>(),
            vec![2, 0],
        );
        assert_eq!(callbacks.len(), 1);
        assert_eq!(callbacks[0].evaluation, result.evaluation);
        assert_eq!(
            (
                callbacks[0].units_done,
                callbacks[0].units_total,
                callbacks[0].complete
            ),
            (result.units_done, result.units_total, result.complete),
        );
        assert_eq!(
            callbacks[0]
                .ranked
                .iter()
                .map(|candidate| candidate.samples)
                .collect::<Vec<_>>(),
            vec![2, 0],
        );
        assert_eq!(game, before);
    }

    #[test]
    fn second_mode_discards_a_cancelled_hidden_wager_column_before_publishing() {
        let mut game = round_three_game(1);
        let before = game.clone();
        // The first four nodes fill one hidden-wager column. The fifth starts the next
        // column, then cancellation prevents its second candidate and drops the column.
        let mut control = PolicyControl::for_nodes(5);
        let mut callbacks = Vec::new();
        let result = search_with_test_control(
            &mut game,
            config(
                SearchMode::Second {
                    opponent_hand_index: 2,
                },
                Duration::from_secs(1),
            ),
            &mut control,
            |snapshot| callbacks.push(snapshot.clone()),
        );
        assert_eq!(control.nodes(), 5);
        assert!(!result.complete);
        assert_eq!((result.units_done, result.units_total), (4, 8));
        assert!(result.ranked.iter().all(|candidate| candidate.samples == 1));
        assert_eq!(callbacks.len(), 1);
        assert_eq!(callbacks[0].evaluation, result.evaluation);
        assert_eq!(
            (
                callbacks[0].units_done,
                callbacks[0].units_total,
                callbacks[0].complete
            ),
            (result.units_done, result.units_total, result.complete),
        );
        assert!(callbacks[0]
            .ranked
            .iter()
            .all(|candidate| candidate.samples == 1));
        assert_eq!(game, before);
    }

    #[test]
    fn blind_second_discards_a_cancelled_card_wager_column_before_publishing() {
        let mut game = round_three_game(1);
        let before = game.clone();
        // Four fixed replies complete the first unknown card/wager column. The fifth node
        // begins the next column, whose remaining replies must be discarded on cancellation.
        let mut control = PolicyControl::for_nodes(5);
        let mut callbacks = Vec::new();
        let result = search_with_test_control(
            &mut game,
            blind_second_config(Duration::from_secs(1)),
            &mut control,
            |snapshot| callbacks.push(snapshot.clone()),
        );

        assert_eq!(control.nodes(), 5);
        assert!(!result.complete);
        assert_eq!((result.units_done, result.units_total), (4, 16));
        assert!(result.ranked.iter().all(|candidate| candidate.samples == 1));
        assert_eq!(callbacks.len(), 1);
        assert_eq!(callbacks[0].evaluation, result.evaluation);
        assert_eq!(
            (
                callbacks[0].units_done,
                callbacks[0].units_total,
                callbacks[0].complete
            ),
            (result.units_done, result.units_total, result.complete),
        );
        assert!(callbacks[0]
            .ranked
            .iter()
            .all(|candidate| candidate.samples == 1));
        assert_eq!(game, before);
    }

    #[test]
    fn ranking_uses_displayed_percent_then_visible_ties_and_cost() {
        let expensive = RankedMove {
            move_: AdvisorMove {
                hand_index: 0,
                pillz: 5,
                fury: false,
            },
            average: 0.104,
            worst: 0.0,
            best: 0.0,
            samples: 1,
            kos: 0,
            koed: 0,
            hidden_outcomes: Vec::new(),
        };
        let cheap = RankedMove {
            move_: AdvisorMove {
                hand_index: 1,
                pillz: 1,
                fury: false,
            },
            average: 0.100,
            worst: 0.0,
            best: 0.0,
            samples: 1,
            kos: 0,
            koed: 0,
            hidden_outcomes: Vec::new(),
        };
        assert_eq!(
            displayed_percent(expensive.average),
            displayed_percent(cheap.average)
        );
        let mut ranked = vec![expensive, cheap.clone()];
        ranked.sort_by(|left, right| {
            compare_ranked(left, right, EvaluationKind::ExactContinuationPolicy)
        });
        assert_eq!(ranked[0], cheap);

        let mut game = test_game(20, 0, (7, 3), (6, 2));
        let first = search(
            &mut game,
            config(SearchMode::First, Duration::from_secs(1)),
            |_| {},
        );
        let second = search(
            &mut game,
            config(SearchMode::First, Duration::from_secs(1)),
            |_| {},
        );
        assert_eq!(first.ranked, second.ranked);
    }

    #[test]
    fn opening_ranking_ignores_the_observed_floor_but_exact_ranking_keeps_worst() {
        let higher_floor = RankedMove {
            move_: AdvisorMove {
                hand_index: 0,
                pillz: 2,
                fury: false,
            },
            average: 0.2,
            worst: 0.0,
            best: 0.8,
            samples: 2,
            kos: 0,
            koed: 0,
            hidden_outcomes: Vec::new(),
        };
        let knockout = RankedMove {
            move_: AdvisorMove {
                hand_index: 1,
                pillz: 2,
                fury: false,
            },
            average: 0.2,
            worst: -1.0,
            best: 1.0,
            samples: 2,
            kos: 1,
            koed: 0,
            hidden_outcomes: Vec::new(),
        };
        let mut opening = vec![higher_floor.clone(), knockout.clone()];
        opening.sort_by(|left, right| compare_ranked(left, right, EvaluationKind::OpeningEstimate));
        assert_eq!(opening[0], knockout);

        let mut exact = vec![higher_floor.clone(), knockout];
        exact.sort_by(|left, right| {
            compare_ranked(left, right, EvaluationKind::ExactContinuationPolicy)
        });
        assert_eq!(exact[0], higher_floor);
    }
}
