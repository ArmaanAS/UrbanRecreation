//! Responsive current-round search for the replay-grounded advisor.
//!
//! Every current-round pairing is resolved by the real [`CombatStatDiagnosticV1`] engine,
//! and every nonterminal pairing is solved to the end of the match by the exact
//! information-aware continuation in `policy`, round one included. Round one differs only in
//! weighting the opponent's reply by the captured opening prior.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering as AtomicOrdering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crate::engine::{
    BaseRulesRoundInput, BaseRulesSelection, ByPlayer, CombatStatDiagnosticV1, MatchStatus,
    PlayerId, FURY_COST,
};

use super::policy::{continuation_value, ExactValue, PolicyControl};

/// Semantic identity of the live recommendation policy, including the literal opening prior
/// below. Bump this whenever ranking, continuation, or opening-weight semantics change in a
/// way that can change a recommendation. Revision 3 solves round one exactly and weights it
/// by the recounted opponent-only prior.
pub const ADVISOR_POLICY_SEMANTIC_REVISION_V1: u16 = 3;

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

/// Which exact evaluation a root ran. Both solve every nonterminal sample to the end of the
/// match with the conservative continuation policy, so the Worst column is always a
/// guarantee over the opponent's current choice; they differ only in how that choice is
/// weighted.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EvaluationKind {
    /// The opening root: the opponent's reply is weighted by the captured opening prior,
    /// which is empirical information about what opponents actually open with.
    ExactOpeningPolicy,
    /// Roots in rounds two through four: every opposing choice weighs the same.
    ExactContinuationPolicy,
}

impl EvaluationKind {
    /// True when the opponent's current reply is weighted by the captured opening prior
    /// rather than uniformly. That is a property of the round, not of the evaluator.
    pub const fn weights_by_opening_prior(self) -> bool {
        matches!(self, Self::ExactOpeningPolicy)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RankedMove {
    pub move_: AdvisorMove,
    /// Mean in our frame: captured-reply weighted in the opening, uniform afterwards.
    pub average: f64,
    /// Lowest complete opposing sample seen for this move, also in our frame: the
    /// game-theoretic Worst once every opposing choice is in.
    pub worst: f64,
    /// Highest complete opposing sample seen for this move, also in our frame.
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

// The opponent's round-one reply is weighted by what opponents actually opened with. One
// Laplace observation is added to every legal reply so an unseen wager remains possible. The
// TypeScript advisor carries the same table, and tests/solver/OpeningPrior.test.ts holds the
// two literals equal.
// BEGIN OPENING_REPLY_COUNTS (generated by `deno task opening-prior --write`)
// 380 captured opponent round-one plays from 380 of the 383 captures
// up to 2026-09-23T20:58:52.100Z, counted 2026-09-26 by `deno task opening-prior`: every
// opponent round-one move, every room and battle rule, both movers, keyed by engine pillz
// (server pillzUsed - 1) and the Fury flag. Rerun that task to refresh it; adding
// captures does not change it by itself.
const OPENING_REPLY_COUNTS: &[((u16, bool), u16)] = &[
    ((0, false), 72),
    ((0, true), 1),
    ((1, false), 27),
    ((2, false), 42),
    ((3, false), 49),
    ((4, false), 60),
    ((4, true), 1),
    ((5, false), 50),
    ((6, false), 39),
    ((6, true), 1),
    ((7, false), 24),
    ((7, true), 1),
    ((8, false), 8),
    ((8, true), 1),
    ((9, false), 2),
    ((9, true), 1),
    ((10, false), 1),
];
// END OPENING_REPLY_COUNTS

/// Captured plays behind [`OPENING_REPLY_COUNTS`], before smoothing.
pub const OPENING_REPLY_PLAYS: u32 = {
    let mut total = 0;
    let mut index = 0;
    while index < OPENING_REPLY_COUNTS.len() {
        total += OPENING_REPLY_COUNTS[index].1 as u32;
        index += 1;
    }
    total
};

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

/// The worker count a root search uses unless the caller names one: one per hardware thread
/// the operating system reports, and one when it cannot say.
pub fn default_search_threads() -> NonZeroUsize {
    thread::available_parallelism().unwrap_or(NonZeroUsize::MIN)
}

/// Evaluates the current round until every pairing is complete or the time budget expires,
/// on [`default_search_threads`] workers.
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
    search_with_threads(game, config, default_search_threads(), progress)
}

/// [`search`] on exactly `threads` workers. One thread evaluates every block inline on the
/// caller's own game, in block order; more threads each own a clone of the root. A complete
/// result is bit-identical whatever the count, because completed blocks are always folded
/// into the candidates in ascending block order rather than in completion order.
///
/// The callback always runs on the calling thread. A partial result is a set of whole
/// blocks: a prefix with one thread, but with several it can be any subset, because a later
/// block can finish before an earlier one is cut by the deadline.
pub fn search_with_threads(
    game: &mut CombatStatDiagnosticV1,
    config: SearchConfig,
    threads: NonZeroUsize,
    progress: impl FnMut(&SearchSnapshot),
) -> SearchSnapshot {
    let started = Instant::now();
    let mut policy_control = PolicyControl::for_budget(started, config.budget);
    search_with_control(
        game,
        config,
        threads,
        started,
        &mut policy_control,
        progress,
    )
}

#[cfg(test)]
fn search_with_test_control(
    game: &mut CombatStatDiagnosticV1,
    config: SearchConfig,
    policy_control: &mut PolicyControl,
    progress: impl FnMut(&SearchSnapshot),
) -> SearchSnapshot {
    search_with_control(
        game,
        config,
        NonZeroUsize::MIN,
        Instant::now(),
        policy_control,
        progress,
    )
}

fn search_with_control(
    game: &mut CombatStatDiagnosticV1,
    config: SearchConfig,
    threads: NonZeroUsize,
    started: Instant,
    policy_control: &mut PolicyControl,
    mut progress: impl FnMut(&SearchSnapshot),
) -> SearchSnapshot {
    let evaluation = if game.position().rounds_played == 0 {
        EvaluationKind::ExactOpeningPolicy
    } else {
        EvaluationKind::ExactContinuationPolicy
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
    let mut candidates: Vec<_> = our_moves.iter().copied().map(Candidate::new).collect();

    // A publication is transactional. First mode commits a whole reply row; Second and
    // BlindSecond modes commit one whole hidden-opponent column. A deadline or policy
    // cancellation inside either block discards its buffered samples, so ranked candidates
    // never contain incomparable partial blocks and `units_done` always counts published
    // work.
    let (block_count, block_len) = match config.mode {
        SearchMode::First => (our_moves.len(), opponent_moves.len()),
        SearchMode::Second { .. } | SearchMode::BlindSecond => {
            (opponent_moves.len(), our_moves.len())
        }
    };
    let pair = |block: usize, unit: usize| match config.mode {
        SearchMode::First => (our_moves[block], opponent_moves[unit]),
        SearchMode::Second { .. } | SearchMode::BlindSecond => {
            (our_moves[unit], opponent_moves[block])
        }
    };
    let evaluate_block = |game: &mut CombatStatDiagnosticV1,
                          policy_control: &mut PolicyControl,
                          block: usize|
     -> Option<Vec<Sample>> {
        let mut samples = Vec::with_capacity(block_len);
        for unit in 0..block_len {
            if started.elapsed() >= config.budget {
                return None;
            }
            let (our_move, opponent_move) = pair(block, unit);
            samples.push(evaluate_pair(
                game,
                config,
                our_move,
                opponent_move,
                policy_control,
            )?);
        }
        Some(samples)
    };
    let retain_hidden_outcomes = matches!(config.mode, SearchMode::Second { .. });
    let fold = |candidates: &mut [Candidate], block: usize, samples: &[Sample]| {
        for (unit, &sample) in samples.iter().enumerate() {
            let (_, opponent_move) = pair(block, unit);
            let candidate = match config.mode {
                SearchMode::First => &mut candidates[block],
                SearchMode::Second { .. } | SearchMode::BlindSecond => &mut candidates[unit],
            };
            candidate.push(sample, sample_weight(evaluation, opponent_move));
            if retain_hidden_outcomes {
                candidate.push_hidden_outcome(opponent_move, sample);
            }
        }
    };

    // `candidates` holds every block below `next_fold`, folded in ascending order. A block
    // that completes ahead of an earlier one waits in `pending` and is folded on top of a
    // copy only for publication, so every published snapshot, and the complete result, sums
    // its samples in the same order as a serial search. With one thread `pending` is always
    // empty.
    let mut next_fold = 0;
    let mut pending: BTreeMap<usize, Vec<Sample>> = BTreeMap::new();
    let mut units_done = 0;
    let mut last_publication: Option<(usize, bool)> = None;
    let published = |candidates: &[Candidate],
                     pending: &BTreeMap<usize, Vec<Sample>>,
                     units_done: usize,
                     complete: bool| {
        let elapsed = started.elapsed();
        if pending.is_empty() {
            return snapshot(
                candidates,
                units_done,
                units_total,
                elapsed,
                complete,
                evaluation,
            );
        }
        let mut view = candidates.to_vec();
        for (&block, samples) in pending {
            fold(&mut view, block, samples);
        }
        snapshot(
            &view,
            units_done,
            units_total,
            elapsed,
            complete,
            evaluation,
        )
    };
    let expired = run_blocks(
        threads,
        block_count,
        game,
        policy_control,
        &evaluate_block,
        |block, samples| {
            pending.insert(block, samples);
            while let Some(samples) = pending.remove(&next_fold) {
                fold(&mut candidates, next_fold, &samples);
                next_fold += 1;
            }
            units_done += block_len;
            // This recommendation is now comparable across its complete reply set, or this
            // exact hidden wager (and, in blind mode, its card) has now been tested against
            // every possible fixed response.
            let complete = units_done == units_total;
            progress(&published(&candidates, &pending, units_done, complete));
            last_publication = Some((units_done, complete));
        },
    );

    let complete = !expired && units_done == units_total;
    let result = published(&candidates, &pending, units_done, complete);
    // Always send a final state for a zero-budget, empty, or between-boundaries stop, but
    // avoid cloning and repainting the final complete matrix twice.
    if last_publication != Some((units_done, complete)) {
        progress(&result);
    }
    result
}

/// Evaluates blocks `0..block_count`, hands each completed one to `completed` on the calling
/// thread, and returns whether a block was cancelled.
///
/// Every worker runs the same loop: claim the next unclaimed block, evaluate it whole, and
/// stop at the first cancelled block, which also stops the others claiming more. With one
/// worker that loop runs inline on the caller's own game and control, so it evaluates and
/// publishes blocks strictly in order and stops exactly where a serial search would. With
/// more, each worker owns a clone of the root game and of the control, whose deadline is the
/// caller's; the root game is never touched, and every worker's nodes are added back.
fn run_blocks<E>(
    threads: NonZeroUsize,
    block_count: usize,
    game: &mut CombatStatDiagnosticV1,
    policy_control: &mut PolicyControl,
    evaluate_block: &E,
    mut completed: impl FnMut(usize, Vec<Sample>),
) -> bool
where
    E: Fn(&mut CombatStatDiagnosticV1, &mut PolicyControl, usize) -> Option<Vec<Sample>> + Sync,
{
    let next_block = AtomicUsize::new(0);
    let cancelled = AtomicBool::new(false);
    let work = |game: &mut CombatStatDiagnosticV1,
                policy_control: &mut PolicyControl,
                completed: &mut dyn FnMut(usize, Vec<Sample>)| {
        while !cancelled.load(AtomicOrdering::Relaxed) {
            let block = next_block.fetch_add(1, AtomicOrdering::Relaxed);
            if block >= block_count {
                break;
            }
            match evaluate_block(game, policy_control, block) {
                Some(samples) => completed(block, samples),
                None => cancelled.store(true, AtomicOrdering::Relaxed),
            }
        }
    };

    let workers = threads.get().min(block_count);
    if workers <= 1 {
        work(game, policy_control, &mut completed);
        return cancelled.into_inner();
    }

    let root: &CombatStatDiagnosticV1 = game;
    let (sender, receiver) = mpsc::channel();
    let nodes = thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                let sender = sender.clone();
                let mut game = root.clone();
                let mut policy_control = policy_control.clone();
                let work = &work;
                let cancelled = &cancelled;
                scope.spawn(move || {
                    let _unwinding = CancelOnPanic(cancelled);
                    let before = policy_control.nodes();
                    work(&mut game, &mut policy_control, &mut |block, samples| {
                        // Only a panicking caller drops the receiver early, and then the
                        // block is not wanted.
                        let _ = sender.send((block, samples));
                    });
                    policy_control.nodes() - before
                })
            })
            .collect();
        drop(sender);
        // Publication stays on the calling thread. The channel closes once every worker
        // has returned or unwound. A panicking callback stops the workers claiming more
        // blocks, so the scope does not wait out the rest of the matrix before unwinding.
        let _unwinding = CancelOnPanic(&cancelled);
        for (block, samples) in receiver {
            completed(block, samples);
        }
        handles
            .into_iter()
            .map(|handle| match handle.join() {
                Ok(nodes) => nodes,
                Err(panic) => std::panic::resume_unwind(panic),
            })
            .sum::<u64>()
    });
    policy_control.absorb_nodes(nodes);
    cancelled.into_inner()
}

/// Stops the other workers claiming blocks when the thread holding it unwinds.
struct CancelOnPanic<'a>(&'a AtomicBool);

impl Drop for CancelOnPanic<'_> {
    fn drop(&mut self) {
        if thread::panicking() {
            self.0.store(true, AtomicOrdering::Relaxed);
        }
    }
}

fn evaluate_pair(
    game: &mut CombatStatDiagnosticV1,
    config: SearchConfig,
    our_move: AdvisorMove,
    opponent_move: AdvisorMove,
    policy_control: &mut PolicyControl,
) -> Option<Sample> {
    let input = round_input(config.first_mover, config.us, our_move, opponent_move);
    let (_, undo) = game
        .make(input)
        .expect("legal advisor moves must execute in a fully admitted match");
    let sample = evaluate(game, config.us, config.first_mover.other(), policy_control);
    game.unmake(undo);
    sample
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
        MatchStatus::Playing => exact_score(continuation_value(
            game,
            us,
            next_first_mover,
            policy_control,
        )?),
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
    if evaluation.weights_by_opening_prior() {
        opening_reply_weight(opponent_move)
    } else {
        1
    }
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
    ranked.sort_by(compare_ranked);
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

fn compare_ranked(left: &RankedMove, right: &RankedMove) -> Ordering {
    // Higher displayed average first. Deliberately ignore sub-percent raw differences:
    // equal-looking rows should be ordered by the visible tie-breaks, the first of which is
    // the game-theoretic Worst.
    let average = displayed_percent(right.average).cmp(&displayed_percent(left.average));
    let worst = displayed_percent(right.worst).cmp(&displayed_percent(left.worst));
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
        CombatStatDiagnosticMatchSpecV1, CombatStatSourcePlanV1, HAND_SIZE,
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

    fn evaluate_now(game: &mut CombatStatDiagnosticV1, us: PlayerId) -> Sample {
        let mut control = PolicyControl::for_budget(Instant::now(), Duration::from_secs(30));
        evaluate(game, us, PlayerId::P2, &mut control).unwrap()
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
    fn opening_reply_table_laplace_smooths_every_wager() {
        // The literal itself is held equal to the TypeScript table and to its recount by
        // tests/solver/OpeningPrior.test.ts; this pins only how it is read.
        assert_eq!(
            OPENING_REPLY_COUNTS
                .iter()
                .map(|(_, count)| u32::from(*count))
                .sum::<u32>(),
            OPENING_REPLY_PLAYS,
        );
        for &((pillz, fury), count) in OPENING_REPLY_COUNTS {
            assert!(count > 0);
            assert_eq!(
                opening_reply_weight(AdvisorMove {
                    hand_index: 3,
                    pillz,
                    fury,
                }),
                count + 1,
            );
        }
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
            config(SearchMode::First, Duration::from_secs(60)),
            |_| first_updates += 1,
        );
        assert!(first.complete);
        assert_eq!(first.ranked.len(), 20);
        assert_eq!(first.units_total, 400);
        assert_eq!(first_updates, 20);
        assert_eq!(first.evaluation, EvaluationKind::ExactOpeningPolicy);
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
                Duration::from_secs(60),
            ),
            |_| second_updates += 1,
        );
        assert!(second.complete);
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
    fn the_opening_is_solved_exactly_and_only_it_is_weighted_by_the_prior() {
        let mut game = test_game(20, 1, (8, 4), (5, 2));

        let exact = search(
            &mut game,
            config(SearchMode::First, Duration::from_secs(30)),
            |_| {},
        );
        assert!(exact.complete);
        assert_eq!(exact.evaluation, EvaluationKind::ExactOpeningPolicy);
        // The opening prior is about what opponents actually open with, so the opening
        // weights its replies by it while solving every leaf.
        assert!(exact.evaluation.weights_by_opening_prior());
        // An exact leaf is a solved match value, so every Worst lands on a win, draw or loss.
        assert!(exact
            .ranked
            .iter()
            .all(|row| row.worst == -1.0 || row.worst == 0.0 || row.worst == 1.0));

        // Once a round has been played every opposing choice weighs the same.
        let mut played = game.clone();
        played
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
        let result = search(
            &mut played,
            config(SearchMode::First, Duration::from_secs(30)),
            |_| {},
        );
        assert_eq!(result.evaluation, EvaluationKind::ExactContinuationPolicy);
        assert!(!result.evaluation.weights_by_opening_prior());
    }

    #[test]
    fn an_exact_opening_restores_the_root_and_ranks_by_its_guaranteed_worst() {
        let mut game = test_game(20, 1, (8, 4), (5, 2));
        let before = game.clone();
        let result = search(
            &mut game,
            config(SearchMode::First, Duration::from_secs(30)),
            |_| {},
        );
        assert!(result.complete);
        assert_eq!(game, before);

        // Exact rows break a displayed-percent tie on the guaranteed Worst.
        for pair in result.ranked.windows(2) {
            let (left, right) = (&pair[0], &pair[1]);
            if displayed_percent(left.average) == displayed_percent(right.average) {
                assert!(
                    displayed_percent(left.worst) >= displayed_percent(right.worst),
                    "exact rows must order by Worst within a displayed tie",
                );
            }
        }
    }

    #[test]
    fn opening_search_weights_opponent_wagers_in_both_modes_and_keeps_a_fixed_top() {
        // With one pill, the observed 0-pill reply outweighs the observed 1-pill reply. The
        // stronger P1 cards make saving the pill the stable opening recommendation in either
        // visible-information mode.
        let mut game = test_game(20, 1, (8, 4), (5, 2));
        let expected_top = AdvisorMove {
            hand_index: 0,
            pillz: 0,
            fury: false,
        };
        let first = search(
            &mut game,
            config(SearchMode::First, Duration::from_secs(30)),
            |_| {},
        );
        let second = search(
            &mut game,
            config(
                SearchMode::Second {
                    opponent_hand_index: 0,
                },
                Duration::from_secs(30),
            ),
            |_| {},
        );
        assert!(first.complete && second.complete);
        assert_eq!(first.evaluation, EvaluationKind::ExactOpeningPolicy);
        assert_eq!(second.evaluation, EvaluationKind::ExactOpeningPolicy);
        // Weighted, not uniform: the opening prior makes some averages non-categorical.
        let prior_weight = |pillz| {
            opening_reply_weight(AdvisorMove {
                hand_index: 0,
                pillz,
                fury: false,
            })
        };
        assert!(prior_weight(0) > prior_weight(1));
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
    fn evaluation_uses_exact_terminal_and_continuation_values() {
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
        let ours = evaluate_now(&mut knockout, PlayerId::P1);
        let theirs = evaluate_now(&mut knockout, PlayerId::P2);
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
        // A nonterminal position is solved to the end of the match, so it is exactly a win,
        // draw or loss for whoever asks.
        for us in [PlayerId::P1, PlayerId::P2] {
            let value = evaluate_now(&mut position, us).value;
            assert!(
                value == -1.0 || value == 0.0 || value == 1.0,
                "{us:?}: {value}"
            );
        }
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
        ranked.sort_by(compare_ranked);
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
    fn ranking_prefers_the_higher_worst_before_a_knockout() {
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
        let mut exact = vec![knockout, higher_floor.clone()];
        exact.sort_by(compare_ranked);
        assert_eq!(exact[0], higher_floor);
    }

    /// Four different cards a side, so the matrix has distinct values, knockouts and
    /// non-integer weighted opening averages rather than one repeated sample.
    fn varied_game(life: u16, pillz: u16) -> CombatStatDiagnosticV1 {
        let hand = |base: u32, stats: [(u16, u16); HAND_SIZE]| {
            std::array::from_fn(|index| card(base + index as u32, stats[index].0, stats[index].1))
        };
        let base_rules = BaseRulesMatchSpec {
            battle_rule_id: 0,
            night: false,
            players: ByPlayer::new(
                BaseRulesPlayerSpec {
                    initial_life: life,
                    initial_pillz: pillz,
                    hand: hand(300, [(8, 4), (5, 6), (7, 2), (3, 7)]),
                },
                BaseRulesPlayerSpec {
                    initial_life: life,
                    initial_pillz: pillz,
                    hand: hand(400, [(6, 5), (7, 3), (4, 6), (9, 1)]),
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

    fn played(
        mut game: CombatStatDiagnosticV1,
        rounds: &[(PlayerId, u8, u16, u8, u16)],
    ) -> CombatStatDiagnosticV1 {
        for &(first_mover, ours, our_pillz, theirs, their_pillz) in rounds {
            game.make(round_input(
                first_mover,
                PlayerId::P1,
                AdvisorMove {
                    hand_index: ours,
                    pillz: our_pillz,
                    fury: false,
                },
                AdvisorMove {
                    hand_index: theirs,
                    pillz: their_pillz,
                    fury: false,
                },
            ))
            .unwrap();
        }
        game
    }

    fn mode_config(mode: SearchMode) -> SearchConfig {
        SearchConfig {
            us: PlayerId::P1,
            first_mover: match mode {
                SearchMode::First => PlayerId::P1,
                SearchMode::Second { .. } | SearchMode::BlindSecond => PlayerId::P2,
            },
            mode,
            budget: Duration::from_secs(600),
        }
    }

    type RowBits = (
        AdvisorMove,
        u64,
        u64,
        u64,
        usize,
        usize,
        usize,
        Vec<(u16, bool, u64, u8)>,
    );

    fn row_bits(row: &RankedMove) -> RowBits {
        (
            row.move_,
            row.average.to_bits(),
            row.worst.to_bits(),
            row.best.to_bits(),
            row.samples,
            row.kos,
            row.koed,
            row.hidden_outcomes
                .iter()
                .map(|outcome| {
                    (
                        outcome.opponent_pillz,
                        outcome.opponent_fury,
                        outcome.value.to_bits(),
                        outcome.flags,
                    )
                })
                .collect(),
        )
    }

    /// Runs one search and returns its result with every published snapshot's shape.
    fn traced(
        game: &mut CombatStatDiagnosticV1,
        config: SearchConfig,
        threads: usize,
    ) -> (SearchSnapshot, Vec<(usize, bool)>) {
        let mut published = Vec::new();
        let result = search_with_threads(game, config, NonZeroUsize::new(threads).unwrap(), |s| {
            published.push((s.units_done, s.complete))
        });
        (result, published)
    }

    #[test]
    fn a_parallel_complete_search_is_bit_identical_to_one_thread() {
        let roots: [(&str, CombatStatDiagnosticV1, EvaluationKind); 3] = [
            (
                "opening",
                varied_game(12, 2),
                EvaluationKind::ExactOpeningPolicy,
            ),
            (
                "round two",
                played(varied_game(12, 5), &[(PlayerId::P1, 0, 1, 0, 1)]),
                EvaluationKind::ExactContinuationPolicy,
            ),
            (
                "round three",
                played(
                    varied_game(12, 7),
                    &[(PlayerId::P1, 0, 1, 0, 1), (PlayerId::P2, 1, 1, 1, 1)],
                ),
                EvaluationKind::ExactContinuationPolicy,
            ),
        ];
        for (label, mut game, evaluation) in roots {
            let unplayed = (0..HAND_SIZE as u8)
                .find(|&slot| !game.position().played[PlayerId::P2][usize::from(slot)])
                .unwrap();
            for mode in [
                SearchMode::First,
                SearchMode::Second {
                    opponent_hand_index: unplayed,
                },
                SearchMode::BlindSecond,
            ] {
                let config = mode_config(mode);
                let before = game.clone();
                let (serial, serial_published) = traced(&mut game, config, 1);
                assert_eq!(game, before, "{label} {mode:?}: one thread moved the root");
                assert!(serial.complete, "{label} {mode:?}");
                assert_eq!(serial.evaluation, evaluation, "{label} {mode:?}");
                assert!(
                    serial
                        .ranked
                        .iter()
                        .any(|row| row.average != serial.ranked[0].average),
                    "{label} {mode:?}: a uniform matrix would not test the fold order",
                );
                for threads in [2, 3, 8] {
                    let (parallel, parallel_published) = traced(&mut game, config, threads);
                    assert_eq!(game, before, "{label} {mode:?} x{threads}: root moved");
                    assert_eq!(
                        (
                            parallel.units_done,
                            parallel.units_total,
                            parallel.complete,
                            parallel.evaluation
                        ),
                        (
                            serial.units_done,
                            serial.units_total,
                            serial.complete,
                            serial.evaluation
                        ),
                        "{label} {mode:?} x{threads}",
                    );
                    assert_eq!(
                        parallel.ranked.iter().map(row_bits).collect::<Vec<_>>(),
                        serial.ranked.iter().map(row_bits).collect::<Vec<_>>(),
                        "{label} {mode:?} x{threads}: ranked rows differ",
                    );
                    // One publication per block, the last of them the complete result, and
                    // no duplicate final: the same shape a serial search publishes.
                    assert_eq!(
                        parallel_published, serial_published,
                        "{label} {mode:?} x{threads}: publications differ",
                    );
                }
            }
        }
    }

    #[test]
    fn remembered_continuation_values_leave_every_complete_result_bit_identical() {
        let roots = [
            ("opening", varied_game(12, 3)),
            (
                "round two",
                played(varied_game(12, 5), &[(PlayerId::P1, 0, 1, 0, 1)]),
            ),
            (
                "round three",
                played(
                    varied_game(12, 7),
                    &[(PlayerId::P1, 0, 1, 0, 1), (PlayerId::P2, 1, 1, 1, 1)],
                ),
            ),
        ];
        for (label, mut game) in roots {
            let unplayed = (0..HAND_SIZE as u8)
                .find(|&slot| !game.position().played[PlayerId::P2][usize::from(slot)])
                .unwrap();
            for mode in [
                SearchMode::First,
                SearchMode::Second {
                    opponent_hand_index: unplayed,
                },
                SearchMode::BlindSecond,
            ] {
                let config = mode_config(mode);
                let before = game.clone();
                let mut run = |mut control: PolicyControl| {
                    let result = search_with_control(
                        &mut game,
                        config,
                        NonZeroUsize::MIN,
                        Instant::now(),
                        &mut control,
                        |_| {},
                    );
                    (result, control.nodes())
                };
                let (remembered, remembered_nodes) = run(PolicyControl::for_nodes(u64::MAX));
                let (recomputed, recomputed_nodes) =
                    run(PolicyControl::for_nodes(u64::MAX).without_cache());
                assert_eq!(game, before, "{label} {mode:?}: the root moved");
                assert!(
                    remembered.complete && recomputed.complete,
                    "{label} {mode:?}"
                );
                assert_eq!(
                    remembered.ranked.iter().map(row_bits).collect::<Vec<_>>(),
                    recomputed.ranked.iter().map(row_bits).collect::<Vec<_>>(),
                    "{label} {mode:?}: remembering a value changed it",
                );
                assert!(
                    remembered_nodes < recomputed_nodes,
                    "{label} {mode:?}: no position was reached twice \
                     ({remembered_nodes} vs {recomputed_nodes})",
                );
            }
        }
    }

    /// The final result carries a later `elapsed` than the publication it repeats.
    fn assert_same_result(published: &SearchSnapshot, result: &SearchSnapshot) {
        assert_eq!(
            (
                published.units_done,
                published.units_total,
                published.complete
            ),
            (result.units_done, result.units_total, result.complete),
        );
        assert_eq!(
            published.ranked.iter().map(row_bits).collect::<Vec<_>>(),
            result.ranked.iter().map(row_bits).collect::<Vec<_>>(),
        );
    }

    /// Whole-block shape of a partial result: First rows are empty or full, Second and
    /// BlindSecond candidates all carry the same completed columns.
    fn assert_whole_blocks(snapshot: &SearchSnapshot, mode: SearchMode, replies: usize) {
        let samples: usize = snapshot.ranked.iter().map(|row| row.samples).sum();
        assert_eq!(samples, snapshot.units_done);
        match mode {
            SearchMode::First => assert!(snapshot
                .ranked
                .iter()
                .all(|row| row.samples == 0 || row.samples == replies)),
            SearchMode::Second { .. } | SearchMode::BlindSecond => {
                let columns = snapshot.ranked[0].samples;
                assert!(snapshot.ranked.iter().all(|row| row.samples == columns));
            }
        }
    }

    #[test]
    fn a_deadline_cut_parallel_search_publishes_only_whole_blocks_and_restores_the_root() {
        let mut game = varied_game(12, 4);
        let before = game.clone();
        let complete = search_with_threads(
            &mut game,
            mode_config(SearchMode::Second {
                opponent_hand_index: 1,
            }),
            NonZeroUsize::MIN,
            |_| {},
        );
        let exact_outcome = |move_: AdvisorMove, outcome: &HiddenOutcome| {
            complete
                .ranked
                .iter()
                .find(|row| row.move_ == move_)
                .unwrap()
                .hidden_outcomes
                .iter()
                .any(|full| {
                    (
                        full.opponent_pillz,
                        full.opponent_fury,
                        full.value.to_bits(),
                        full.flags,
                    ) == (
                        outcome.opponent_pillz,
                        outcome.opponent_fury,
                        outcome.value.to_bits(),
                        outcome.flags,
                    )
                })
        };

        for mode in [
            SearchMode::First,
            SearchMode::Second {
                opponent_hand_index: 1,
            },
            SearchMode::BlindSecond,
        ] {
            let config = mode_config(mode);
            let replies = match mode {
                SearchMode::First => legal_moves(&game, PlayerId::P2).len(),
                _ => 0,
            };
            // Each worker's copy of the control stops after a fifth of the round inputs a
            // complete search executes, which cuts the matrix at several places at once.
            let mut full = PolicyControl::for_nodes(u64::MAX);
            search_with_control(
                &mut game,
                config,
                NonZeroUsize::MIN,
                Instant::now(),
                &mut full,
                |_| {},
            );
            let mut control = PolicyControl::for_nodes(full.nodes() / 5);
            let mut published = Vec::new();
            let result = search_with_control(
                &mut game,
                config,
                NonZeroUsize::new(4).unwrap(),
                Instant::now(),
                &mut control,
                |snapshot| published.push(snapshot.clone()),
            );
            assert_eq!(game, before, "{mode:?}: the root moved");
            assert!(!result.complete, "{mode:?}");
            assert!(result.units_done > 0, "{mode:?}");
            assert!(
                control.nodes() > full.nodes() / 5,
                "{mode:?}: the workers' nodes were not counted"
            );
            assert!(!published.is_empty());
            for snapshot in &published {
                assert_whole_blocks(snapshot, mode, replies);
                assert!(!snapshot.complete);
            }
            assert!(published
                .windows(2)
                .all(|pair| pair[0].units_done < pair[1].units_done));
            // The last publication already is the result, so it is not sent twice.
            assert_same_result(published.last().unwrap(), &result);
            assert_whole_blocks(&result, mode, replies);
            if let SearchMode::Second { .. } = mode {
                // Whatever subset of columns finished, each one holds the exact value a
                // complete search found for that hidden wager.
                assert!(result.ranked.iter().all(|row| {
                    row.hidden_outcomes.len() == row.samples
                        && row
                            .hidden_outcomes
                            .iter()
                            .all(|outcome| exact_outcome(row.move_, outcome))
                }));
            }
        }

        // A wall-clock deadline is honoured by every worker and cut blocks are discarded.
        for mode in [SearchMode::First, SearchMode::BlindSecond] {
            let config = SearchConfig {
                budget: Duration::from_millis(30),
                ..mode_config(mode)
            };
            let mut published = Vec::new();
            let result = search_with_threads(
                &mut varied_game(12, 6),
                config,
                NonZeroUsize::new(4).unwrap(),
                |snapshot| published.push(snapshot.clone()),
            );
            assert!(!result.complete);
            assert!(
                result.elapsed < Duration::from_secs(5),
                "{:?}",
                result.elapsed
            );
            assert_same_result(published.last().unwrap(), &result);
            for snapshot in &published {
                assert_whole_blocks(
                    snapshot,
                    mode,
                    legal_moves(&varied_game(12, 6), PlayerId::P2).len(),
                );
            }
        }
    }
}
