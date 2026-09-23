//! Private, allocation-free combat resolution shared by projected diagnostic models.
//!
//! Replay preparation owns admission and predicates. This module receives only compact,
//! already-admitted effects for the two selected cards.

use super::clan_bonus_diagnostic::{
    DiagnosticAffectedSideV1, DiagnosticCombatEffectV1, DiagnosticCombatStatV1,
    DiagnosticMagnitudeV1, DiagnosticStatOperationV1,
};
use super::{
    BaseRulesCardResult, ByPlayer, PlayerId, PostRoundEffect, PostRoundPlan, PostRoundSourceEffect,
    PreparedSelection, ValidatedSelection, FURY_DAMAGE, MAX_ROUNDS,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CombatResolutionArithmeticStage {
    EffectMagnitude,
    Power,
    Damage,
    Attack,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CombatResolutionError {
    pub player: PlayerId,
    pub stage: CombatResolutionArithmeticStage,
}

#[derive(Clone, Copy, Default)]
pub(super) struct ResolutionSourcePlan {
    pub effect: Option<DiagnosticCombatEffectV1>,
    pub post_round: Option<PostRoundSourceEffect>,
    pub support_count: u16,
    /// The Brawl multiplier: how many distinct characters in the *opposing* hand share the
    /// opposing selected card's effective clan. Derived one layer up, in
    /// `prepare_combat_stat_diagnostic`, which is the lowest layer that holds both hands -
    /// this one is handed only the two selected cards. It is a property of the opposing
    /// slot, not of the owner's, which is why it cannot be a per-owner-slot value.
    pub anti_support_count: u16,
}

#[derive(Clone, Copy, Default)]
pub(super) struct ResolutionCardPlan {
    pub ability: ResolutionSourcePlan,
    pub bonus: ResolutionSourcePlan,
}

#[derive(Clone, Copy)]
pub(super) struct PreparedCombatResolution {
    pub selections: ByPlayer<PreparedSelection>,
    pub post_round: ByPlayer<PostRoundPlan>,
}

/// A set of combat stats. Used for two independent things: the stats whose opposing
/// modifiers a `Cancel Opp. ... Modif.` source removes, and the stats a `Protection`
/// source refuses to let the opposing card reduce.
#[derive(Clone, Copy, Default)]
struct StatMask {
    attack: bool,
    damage: bool,
    power: bool,
}

impl StatMask {
    fn insert(&mut self, stat: DiagnosticCombatStatV1) {
        match stat {
            DiagnosticCombatStatV1::Attack => self.attack = true,
            DiagnosticCombatStatV1::Damage => self.damage = true,
            DiagnosticCombatStatV1::Power => self.power = true,
            DiagnosticCombatStatV1::PowerAndDamage => {
                self.power = true;
                self.damage = true;
            }
        }
    }

    fn contains(self, stat: DiagnosticCombatStatV1) -> bool {
        match stat {
            DiagnosticCombatStatV1::Attack => self.attack,
            DiagnosticCombatStatV1::Damage => self.damage,
            DiagnosticCombatStatV1::Power => self.power,
            DiagnosticCombatStatV1::PowerAndDamage => self.power || self.damage,
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum SourceKind {
    Bonus,
    Ability,
}

const SOURCE_ORDER: [SourceKind; 2] = [SourceKind::Bonus, SourceKind::Ability];

#[derive(Clone, Copy, Default)]
struct SourceLiveness {
    ability: bool,
    bonus: bool,
}

impl SourceLiveness {
    fn get(self, kind: SourceKind) -> bool {
        match kind {
            SourceKind::Ability => self.ability,
            SourceKind::Bonus => self.bonus,
        }
    }

    fn set(&mut self, kind: SourceKind, live: bool) {
        match kind {
            SourceKind::Ability => self.ability = live,
            SourceKind::Bonus => self.bonus = live,
        }
    }
}

fn source_for(card: ResolutionCardPlan, kind: SourceKind) -> ResolutionSourcePlan {
    match kind {
        SourceKind::Ability => card.ability,
        SourceKind::Bonus => card.bonus,
    }
}

fn stop_target(source: ResolutionSourcePlan) -> Option<SourceKind> {
    match source.effect {
        Some(DiagnosticCombatEffectV1::StopOpponentAbility) => Some(SourceKind::Ability),
        Some(DiagnosticCombatEffectV1::StopOpponentBonus) => Some(SourceKind::Bonus),
        _ => None,
    }
}

fn source_liveness(selected_plans: ByPlayer<ResolutionCardPlan>) -> ByPlayer<SourceLiveness> {
    let mut live = ByPlayer::new(
        SourceLiveness {
            ability: source_is_live(selected_plans[PlayerId::P1].ability),
            bonus: source_is_live(selected_plans[PlayerId::P1].bonus),
        },
        SourceLiveness {
            ability: source_is_live(selected_plans[PlayerId::P2].ability),
            bonus: source_is_live(selected_plans[PlayerId::P2].bonus),
        },
    );
    // PRE4 is a dependency resolution, not an arbitrary player-order application.  A
    // pending Stop waits for any opposing pending Stop which can cancel its own source;
    // after a source becomes dead its pending Stop is discarded.  This mirrors
    // TypeScript Events.executeCancels exactly for the compact one-effect-per-source
    // projection.  Four source slots keep the algorithm allocation-free.
    let mut pending = ByPlayer::new([false; 2], [false; 2]);
    for player in PlayerId::ALL {
        for kind in SOURCE_ORDER {
            pending[player][kind_index(kind)] = live[player].get(kind)
                && stop_target(source_for(selected_plans[player], kind)).is_some();
        }
    }

    while has_pending(pending) {
        // A resolved Stop can make a later node inert.  Drop it before considering
        // dependencies, which also releases any source it looked able to block.
        for player in PlayerId::ALL {
            for kind in SOURCE_ORDER {
                let index = kind_index(kind);
                if pending[player][index] && !live[player].get(kind) {
                    pending[player][index] = false;
                }
            }
        }
        if !has_pending(pending) {
            break;
        }

        let mut chosen = None;
        for player in PlayerId::ALL {
            for kind in SOURCE_ORDER {
                let index = kind_index(kind);
                if !pending[player][index] {
                    continue;
                }
                let opponent = player.other();
                let blocked = SOURCE_ORDER.into_iter().any(|blocker_kind| {
                    pending[opponent][kind_index(blocker_kind)]
                        && stop_target(source_for(selected_plans[opponent], blocker_kind))
                            == Some(kind)
                });
                if !blocked {
                    chosen = Some((player, kind));
                    break;
                }
            }
            if chosen.is_some() {
                break;
            }
        }

        // A true mutual-stop cycle has no dependency-free source.  Preserve the
        // TypeScript fallback: internal P1 first, Bonus before Ability.
        let (player, kind) = chosen.unwrap_or_else(|| {
            for player in PlayerId::ALL {
                for kind in SOURCE_ORDER {
                    if pending[player][kind_index(kind)] {
                        return (player, kind);
                    }
                }
            }
            unreachable!("a pending control must exist")
        });
        pending[player][kind_index(kind)] = false;
        if let Some(target) = stop_target(source_for(selected_plans[player], kind)) {
            live[player.other()].set(target, false);
        }
    }

    // Protection resolves after the Stop graph, exactly as TypeScript applies PRE3 after
    // PRE4: a source that survived the round's Stops restores its own card's protected
    // source. A restored source does not get to fire a Stop of its own - by the time it
    // comes back the Stop graph has already run - and a protecting source that was itself
    // stopped protects nothing, which is what keeps a self-referential Protection inert.
    // Battle 876752 r0 has Lady Ametia Cr keep "+1 Power Per Life Left" at 13 Power
    // through Mavi's Stop Opp. Ability, and 964088 r1 has El Tortillo keep "+1 Attack Per
    // Life Left" through Miyo's Stop Opp. Bonus.
    let resolved = live;
    for player in PlayerId::ALL {
        for kind in SOURCE_ORDER {
            if !resolved[player].get(kind) {
                continue;
            }
            let protected = match source_for(selected_plans[player], kind).effect {
                Some(DiagnosticCombatEffectV1::ProtectOwnAbility) => SourceKind::Ability,
                Some(DiagnosticCombatEffectV1::ProtectOwnBonus) => SourceKind::Bonus,
                _ => continue,
            };
            live[player].set(
                protected,
                source_is_live(source_for(selected_plans[player], protected)),
            );
        }
    }

    live
}

const fn kind_index(kind: SourceKind) -> usize {
    match kind {
        SourceKind::Bonus => 0,
        SourceKind::Ability => 1,
    }
}

fn has_pending(pending: ByPlayer<[bool; 2]>) -> bool {
    pending[PlayerId::P1].iter().any(|pending| *pending)
        || pending[PlayerId::P2].iter().any(|pending| *pending)
}

fn add_cancellation(mask: &mut StatMask, source: ResolutionSourcePlan) {
    if let Some(DiagnosticCombatEffectV1::CancelOpponentCombatStatModifiers { stat }) =
        source.effect
    {
        mask.insert(stat);
    }
}

fn add_protection(mask: &mut StatMask, source: ResolutionSourcePlan) {
    if let Some(DiagnosticCombatEffectV1::ProtectOwnCombatStat { stat }) = source.effect {
        mask.insert(stat);
    }
}

/// `Copy: Opp. <stat>` overwrites rather than adds, so it runs before every modifier and
/// reads the opposing card's printed value, not its resolved one. Battle 1025031 r0 pins
/// both halves: Natasha copies Nantosuelte's printed 4 Damage - not the 7 its Asymmetry
/// bonus had made of it - and her own `Damage +2` then produces the reported 6. Since both
/// sides read printed values, two simultaneous copies cannot depend on their order.
///
/// `<stat> Exchange` is the same overwrite applied to both cards: each takes the other's
/// printed value. Every selected Exchange round in the corpus fits that one model - a swap
/// of printed values first, then own increases, then opposing reductions. 1087884/1 pins
/// the order against a reduction (Sue's `-1 Opp Power And Damage` takes her swapped 6 to 5,
/// where reducing first would leave 6 against 4), 1080007/2 an opposing increase landing on
/// the swapped value, and 948108/3 that the opposing `Protection: Power And Damage` does
/// not refuse the swap. As in the reference, an opposing Cancel of the stat skips the whole
/// swap rather than half of it.
fn apply_printed_stat_copy(
    origin: PlayerId,
    source: ResolutionSourcePlan,
    opponent_cancellation: StatMask,
    printed: ByPlayer<(u16, u16)>,
    power: &mut ByPlayer<u16>,
    damage: &mut ByPlayer<u16>,
) {
    let (stat, exchange) = match source.effect {
        Some(DiagnosticCombatEffectV1::CopyOpponentPrintedCombatStat { stat }) => (stat, false),
        Some(DiagnosticCombatEffectV1::ExchangePrintedCombatStat { stat }) => (stat, true),
        _ => return,
    };
    let (own_power, own_damage) = printed[origin];
    let (opponent_power, opponent_damage) = printed[origin.other()];
    if matches!(
        stat,
        DiagnosticCombatStatV1::Power | DiagnosticCombatStatV1::PowerAndDamage
    ) && !opponent_cancellation.contains(DiagnosticCombatStatV1::Power)
    {
        power[origin] = opponent_power;
        if exchange {
            power[origin.other()] = own_power;
        }
    }
    if matches!(
        stat,
        DiagnosticCombatStatV1::Damage | DiagnosticCombatStatV1::PowerAndDamage
    ) && !opponent_cancellation.contains(DiagnosticCombatStatV1::Damage)
    {
        damage[origin] = opponent_damage;
        if exchange {
            damage[origin.other()] = own_damage;
        }
    }
}

pub(super) fn prepare_combat_resolution(
    validated: ByPlayer<ValidatedSelection>,
    selected_plans: ByPlayer<ResolutionCardPlan>,
    rounds_played: u8,
) -> Result<ByPlayer<PreparedSelection>, CombatResolutionError> {
    Ok(
        prepare_combat_resolution_with_post_round(validated, selected_plans, rounds_played)?
            .selections,
    )
}

pub(super) fn prepare_combat_resolution_with_post_round(
    validated: ByPlayer<ValidatedSelection>,
    selected_plans: ByPlayer<ResolutionCardPlan>,
    rounds_played: u8,
) -> Result<PreparedCombatResolution, CombatResolutionError> {
    let opponent_stars = ByPlayer::new(
        u16::from(validated[PlayerId::P2].card.key.level),
        u16::from(validated[PlayerId::P1].card.key.level),
    );
    let live = source_liveness(selected_plans);

    let mut cancellations = ByPlayer::new(StatMask::default(), StatMask::default());
    // Protection only ever refuses an opposing reduction. It removes nothing already
    // applied, so it needs no ordering of its own: the opposing decrease simply does not
    // happen. 949439 r0 keeps Nebula at 7 Power against Olga Cr's "-2 Opp Power, Min 5",
    // and 924320 r1 keeps its 4 Damage against Donald's "-3 Opp Damage, Min 2".
    let mut protections = ByPlayer::new(StatMask::default(), StatMask::default());
    for player in PlayerId::ALL {
        if live[player].ability {
            add_cancellation(&mut cancellations[player], selected_plans[player].ability);
            add_protection(&mut protections[player], selected_plans[player].ability);
        }
        if live[player].bonus {
            add_cancellation(&mut cancellations[player], selected_plans[player].bonus);
            add_protection(&mut protections[player], selected_plans[player].bonus);
        }
    }

    let mut power = ByPlayer::new(
        validated[PlayerId::P1].card.power,
        validated[PlayerId::P2].card.power,
    );
    let mut damage = ByPlayer::new(
        validated[PlayerId::P1].card.damage,
        validated[PlayerId::P2].card.damage,
    );

    let printed = ByPlayer::new(
        (
            validated[PlayerId::P1].card.power,
            validated[PlayerId::P1].card.damage,
        ),
        (
            validated[PlayerId::P2].card.power,
            validated[PlayerId::P2].card.damage,
        ),
    );
    for origin in PlayerId::ALL {
        if live[origin].bonus {
            apply_printed_stat_copy(
                origin,
                selected_plans[origin].bonus,
                cancellations[origin.other()],
                printed,
                &mut power,
                &mut damage,
            );
        }
        if live[origin].ability {
            apply_printed_stat_copy(
                origin,
                selected_plans[origin].ability,
                cancellations[origin.other()],
                printed,
                &mut power,
                &mut damage,
            );
        }
    }

    // Source compilation is Bonus then Ability. Own increases retain that stable order.
    for origin in PlayerId::ALL {
        if live[origin].bonus {
            apply_power_damage_effect(
                origin,
                DiagnosticAffectedSideV1::Player,
                DiagnosticStatOperationV1::Increase,
                selected_plans[origin].bonus,
                cancellations[origin.other()],
                StatMask::default(),
                rounds_played,
                opponent_stars[origin],
                &mut power,
                &mut damage,
            )?;
        }
        if live[origin].ability {
            apply_power_damage_effect(
                origin,
                DiagnosticAffectedSideV1::Player,
                DiagnosticStatOperationV1::Increase,
                selected_plans[origin].ability,
                cancellations[origin.other()],
                StatMask::default(),
                rounds_played,
                opponent_stars[origin],
                &mut power,
                &mut damage,
            )?;
        }
    }

    // Server-backed TypeScript semantics stable-sort opponent reductions by descending
    // Min. Equal-Min effects retain source compilation order (Bonus then Ability).
    for origin in PlayerId::ALL {
        let bonus = if live[origin].bonus {
            selected_plans[origin].bonus
        } else {
            ResolutionSourcePlan::default()
        };
        apply_ordered_power_damage_reductions(
            origin,
            bonus,
            live[origin]
                .ability
                .then_some(selected_plans[origin].ability)
                .unwrap_or_default(),
            cancellations[origin.other()],
            protections[origin.other()],
            rounds_played,
            opponent_stars[origin],
            &mut power,
            &mut damage,
        )?;
    }

    // The Attack phase reads the opposing Damage as it stands here: every Power/Damage
    // modifier has run, and Fury has not. Battle 1130726 r3 is what fixes that order -
    // Goran's "+2 Attack Per Opp. Damage" is worth +4 against a Fury Uuber, whose printed
    // 2 Damage becomes 4 only where the damage is dealt.
    let pre_fury_damage = damage;

    // Fury is added after Power/Damage modifiers.
    for player in PlayerId::ALL {
        if validated[player].selection.fury {
            damage[player] =
                damage[player]
                    .checked_add(FURY_DAMAGE)
                    .ok_or(CombatResolutionError {
                        player,
                        stage: CombatResolutionArithmeticStage::Damage,
                    })?;
        }
    }

    let mut attack = ByPlayer::new(0_u32, 0_u32);
    for player in PlayerId::ALL {
        attack[player] = u32::from(power[player])
            .checked_mul(u32::from(validated[player].selection.pillz) + 1)
            .ok_or(CombatResolutionError {
                player,
                stage: CombatResolutionArithmeticStage::Attack,
            })?;
    }

    // Own Attack increases retain Bonus then Ability source order.
    for origin in PlayerId::ALL {
        if live[origin].bonus {
            apply_attack_effect(
                origin,
                DiagnosticAffectedSideV1::Player,
                DiagnosticStatOperationV1::Increase,
                selected_plans[origin].bonus,
                cancellations[origin.other()],
                StatMask::default(),
                rounds_played,
                opponent_stars[origin],
                pre_fury_damage[origin.other()],
                &mut attack,
            )?;
        }
        if live[origin].ability {
            apply_attack_effect(
                origin,
                DiagnosticAffectedSideV1::Player,
                DiagnosticStatOperationV1::Increase,
                selected_plans[origin].ability,
                cancellations[origin.other()],
                StatMask::default(),
                rounds_played,
                opponent_stars[origin],
                pre_fury_damage[origin.other()],
                &mut attack,
            )?;
        }
    }

    // Opponent Attack reductions use the same stable descending-Min ordering.
    for origin in PlayerId::ALL {
        let bonus = if live[origin].bonus {
            selected_plans[origin].bonus
        } else {
            ResolutionSourcePlan::default()
        };
        apply_ordered_attack_reductions(
            origin,
            bonus,
            live[origin]
                .ability
                .then_some(selected_plans[origin].ability)
                .unwrap_or_default(),
            cancellations[origin.other()],
            protections[origin.other()],
            rounds_played,
            opponent_stars[origin],
            pre_fury_damage[origin.other()],
            &mut attack,
        )?;
    }

    let selections = ByPlayer::new(
        finish_selection(
            validated[PlayerId::P1],
            power[PlayerId::P1],
            damage[PlayerId::P1],
            attack[PlayerId::P1],
        ),
        finish_selection(
            validated[PlayerId::P2],
            power[PlayerId::P2],
            damage[PlayerId::P2],
            attack[PlayerId::P2],
        ),
    );
    let post_round = ByPlayer::new(
        PostRoundPlan {
            ability: live[PlayerId::P1]
                .ability
                .then_some(selected_plans[PlayerId::P1].ability.post_round)
                .flatten()
                .map(|effect| {
                    bind_post_round_effect(
                        PlayerId::P1,
                        effect,
                        opponent_stars[PlayerId::P1],
                        selected_plans[PlayerId::P1].ability.anti_support_count,
                    )
                })
                .transpose()?,
            bonus: live[PlayerId::P1]
                .bonus
                .then_some(selected_plans[PlayerId::P1].bonus.post_round)
                .flatten()
                .map(|effect| {
                    bind_post_round_effect(
                        PlayerId::P1,
                        effect,
                        opponent_stars[PlayerId::P1],
                        selected_plans[PlayerId::P1].bonus.anti_support_count,
                    )
                })
                .transpose()?,
        },
        PostRoundPlan {
            ability: live[PlayerId::P2]
                .ability
                .then_some(selected_plans[PlayerId::P2].ability.post_round)
                .flatten()
                .map(|effect| {
                    bind_post_round_effect(
                        PlayerId::P2,
                        effect,
                        opponent_stars[PlayerId::P2],
                        selected_plans[PlayerId::P2].ability.anti_support_count,
                    )
                })
                .transpose()?,
            bonus: live[PlayerId::P2]
                .bonus
                .then_some(selected_plans[PlayerId::P2].bonus.post_round)
                .flatten()
                .map(|effect| {
                    bind_post_round_effect(
                        PlayerId::P2,
                        effect,
                        opponent_stars[PlayerId::P2],
                        selected_plans[PlayerId::P2].bonus.anti_support_count,
                    )
                })
                .transpose()?,
        },
    );
    Ok(PreparedCombatResolution {
        selections,
        post_round,
    })
}

fn source_is_live(source: ResolutionSourcePlan) -> bool {
    source.effect.is_some() || source.post_round.is_some()
}

fn bind_post_round_effect(
    player: PlayerId,
    effect: PostRoundSourceEffect,
    opponent_stars: u16,
    anti_support_count: u16,
) -> Result<PostRoundEffect, CombatResolutionError> {
    let per_anti_support = |per_count: u16| {
        per_count
            .checked_mul(anti_support_count)
            .ok_or(CombatResolutionError {
                player,
                stage: CombatResolutionArithmeticStage::EffectMagnitude,
            })
    };
    match effect {
        PostRoundSourceEffect::Fixed(effect) => Ok(effect),
        PostRoundSourceEffect::ReduceOpponentLifeOnVictoryPerAntiSupport { per_count, minimum } => {
            Ok(PostRoundEffect::ReduceOpponentLifeOnVictory {
                life: per_anti_support(per_count)?,
                minimum,
            })
        }
        PostRoundSourceEffect::ReduceOpponentPillzOnVictoryPerAntiSupport {
            per_count,
            minimum,
        } => Ok(PostRoundEffect::ReduceOpponentPillzOnVictory {
            pillz: per_anti_support(per_count)?,
            minimum,
        }),
        PostRoundSourceEffect::GainPillzOnVictoryPerAntiSupport { per_count, maximum } => {
            let pillz = per_anti_support(per_count)?;
            Ok(if maximum == 0 {
                PostRoundEffect::GainPillzOnVictory(pillz)
            } else {
                PostRoundEffect::GainPillzOnVictoryMax { pillz, maximum }
            })
        }
        PostRoundSourceEffect::ReduceOpponentLifeOnVictoryPerOpponentStars {
            per_star,
            minimum,
        } => {
            let life = per_star
                .checked_mul(opponent_stars)
                .ok_or(CombatResolutionError {
                    player,
                    stage: CombatResolutionArithmeticStage::EffectMagnitude,
                })?;
            Ok(PostRoundEffect::ReduceOpponentLifeOnVictory { life, minimum })
        }
    }
}

fn finish_selection(
    selected: ValidatedSelection,
    power: u16,
    damage: u16,
    attack: u32,
) -> PreparedSelection {
    PreparedSelection {
        slot: selected.slot,
        cost: selected.cost,
        card: selected.card,
        result: BaseRulesCardResult {
            key: selected.card.key,
            hand_slot: selected.slot,
            power,
            damage,
            attack,
            won: false,
        },
    }
}

fn apply_ordered_power_damage_reductions(
    origin: PlayerId,
    bonus: ResolutionSourcePlan,
    ability: ResolutionSourcePlan,
    opponent_cancellation: StatMask,
    target_protection: StatMask,
    rounds_played: u8,
    opponent_stars: u16,
    power: &mut ByPlayer<u16>,
    damage: &mut ByPlayer<u16>,
) -> Result<(), CombatResolutionError> {
    let bonus_min = power_damage_reduction_min(bonus.effect);
    let ability_min = power_damage_reduction_min(ability.effect);
    if ability_min > bonus_min {
        apply_power_damage_effect(
            origin,
            DiagnosticAffectedSideV1::Opponent,
            DiagnosticStatOperationV1::Decrease,
            ability,
            opponent_cancellation,
            target_protection,
            rounds_played,
            opponent_stars,
            power,
            damage,
        )?;
        apply_power_damage_effect(
            origin,
            DiagnosticAffectedSideV1::Opponent,
            DiagnosticStatOperationV1::Decrease,
            bonus,
            opponent_cancellation,
            target_protection,
            rounds_played,
            opponent_stars,
            power,
            damage,
        )
    } else {
        apply_power_damage_effect(
            origin,
            DiagnosticAffectedSideV1::Opponent,
            DiagnosticStatOperationV1::Decrease,
            bonus,
            opponent_cancellation,
            target_protection,
            rounds_played,
            opponent_stars,
            power,
            damage,
        )?;
        apply_power_damage_effect(
            origin,
            DiagnosticAffectedSideV1::Opponent,
            DiagnosticStatOperationV1::Decrease,
            ability,
            opponent_cancellation,
            target_protection,
            rounds_played,
            opponent_stars,
            power,
            damage,
        )
    }
}

fn apply_ordered_attack_reductions(
    origin: PlayerId,
    bonus: ResolutionSourcePlan,
    ability: ResolutionSourcePlan,
    opponent_cancellation: StatMask,
    target_protection: StatMask,
    rounds_played: u8,
    opponent_stars: u16,
    opponent_damage: u16,
    attack: &mut ByPlayer<u32>,
) -> Result<(), CombatResolutionError> {
    let bonus_min = attack_reduction_min(bonus.effect);
    let ability_min = attack_reduction_min(ability.effect);
    if ability_min > bonus_min {
        apply_attack_effect(
            origin,
            DiagnosticAffectedSideV1::Opponent,
            DiagnosticStatOperationV1::Decrease,
            ability,
            opponent_cancellation,
            target_protection,
            rounds_played,
            opponent_stars,
            opponent_damage,
            attack,
        )?;
        apply_attack_effect(
            origin,
            DiagnosticAffectedSideV1::Opponent,
            DiagnosticStatOperationV1::Decrease,
            bonus,
            opponent_cancellation,
            target_protection,
            rounds_played,
            opponent_stars,
            opponent_damage,
            attack,
        )
    } else {
        apply_attack_effect(
            origin,
            DiagnosticAffectedSideV1::Opponent,
            DiagnosticStatOperationV1::Decrease,
            bonus,
            opponent_cancellation,
            target_protection,
            rounds_played,
            opponent_stars,
            opponent_damage,
            attack,
        )?;
        apply_attack_effect(
            origin,
            DiagnosticAffectedSideV1::Opponent,
            DiagnosticStatOperationV1::Decrease,
            ability,
            opponent_cancellation,
            target_protection,
            rounds_played,
            opponent_stars,
            opponent_damage,
            attack,
        )
    }
}

fn power_damage_reduction_min(effect: Option<DiagnosticCombatEffectV1>) -> Option<u16> {
    match effect {
        Some(DiagnosticCombatEffectV1::ModifyCombatStat {
            side: DiagnosticAffectedSideV1::Opponent,
            stat:
                DiagnosticCombatStatV1::Power
                | DiagnosticCombatStatV1::Damage
                | DiagnosticCombatStatV1::PowerAndDamage,
            operation: DiagnosticStatOperationV1::Decrease,
            minimum,
            ..
        }) => Some(minimum.unwrap_or(0)),
        _ => None,
    }
}

fn attack_reduction_min(effect: Option<DiagnosticCombatEffectV1>) -> Option<u16> {
    match effect {
        Some(DiagnosticCombatEffectV1::ModifyCombatStat {
            side: DiagnosticAffectedSideV1::Opponent,
            stat: DiagnosticCombatStatV1::Attack,
            operation: DiagnosticStatOperationV1::Decrease,
            minimum,
            ..
        }) => Some(minimum.unwrap_or(0)),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_power_damage_effect(
    origin: PlayerId,
    expected_side: DiagnosticAffectedSideV1,
    expected_operation: DiagnosticStatOperationV1,
    source: ResolutionSourcePlan,
    opponent_cancellation: StatMask,
    // The stats the card being reduced refuses to have reduced. Empty for an own
    // increase: Protection defends against the opposing character, not its owner.
    target_protection: StatMask,
    rounds_played: u8,
    opponent_stars: u16,
    power: &mut ByPlayer<u16>,
    damage: &mut ByPlayer<u16>,
) -> Result<(), CombatResolutionError> {
    let Some(DiagnosticCombatEffectV1::ModifyCombatStat {
        side,
        stat,
        operation,
        value,
        minimum,
        maximum,
        multiplier,
    }) = source.effect
    else {
        return Ok(());
    };
    if side != expected_side || operation != expected_operation {
        return Ok(());
    }
    let affects_power = matches!(
        stat,
        DiagnosticCombatStatV1::Power | DiagnosticCombatStatV1::PowerAndDamage
    );
    let affects_damage = matches!(
        stat,
        DiagnosticCombatStatV1::Damage | DiagnosticCombatStatV1::PowerAndDamage
    );
    if !affects_power && !affects_damage {
        return Ok(());
    }
    let target = if side == DiagnosticAffectedSideV1::Player {
        origin
    } else {
        origin.other()
    };
    let amount = effect_amount(
        origin,
        value,
        multiplier,
        source.support_count,
        source.anti_support_count,
        rounds_played,
        opponent_stars,
        0,
    )?;
    let protected = |stat| {
        expected_side == DiagnosticAffectedSideV1::Opponent && target_protection.contains(stat)
    };
    if affects_power
        && !opponent_cancellation.contains(DiagnosticCombatStatV1::Power)
        && !protected(DiagnosticCombatStatV1::Power)
    {
        power[target] = apply_u16_modifier(
            origin,
            CombatResolutionArithmeticStage::Power,
            power[target],
            operation,
            amount,
            minimum,
            maximum,
        )?;
    }
    if affects_damage
        && !opponent_cancellation.contains(DiagnosticCombatStatV1::Damage)
        && !protected(DiagnosticCombatStatV1::Damage)
    {
        damage[target] = apply_u16_modifier(
            origin,
            CombatResolutionArithmeticStage::Damage,
            damage[target],
            operation,
            amount,
            minimum,
            maximum,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn apply_attack_effect(
    origin: PlayerId,
    expected_side: DiagnosticAffectedSideV1,
    expected_operation: DiagnosticStatOperationV1,
    source: ResolutionSourcePlan,
    opponent_cancellation: StatMask,
    // As above: consulted only for a reduction aimed at the opposing card.
    target_protection: StatMask,
    rounds_played: u8,
    opponent_stars: u16,
    opponent_damage: u16,
    attack: &mut ByPlayer<u32>,
) -> Result<(), CombatResolutionError> {
    let Some(DiagnosticCombatEffectV1::ModifyCombatStat {
        side,
        stat: DiagnosticCombatStatV1::Attack,
        operation,
        value,
        minimum,
        maximum,
        multiplier,
    }) = source.effect
    else {
        return Ok(());
    };
    if side != expected_side
        || operation != expected_operation
        || opponent_cancellation.contains(DiagnosticCombatStatV1::Attack)
        || (expected_side == DiagnosticAffectedSideV1::Opponent
            && target_protection.contains(DiagnosticCombatStatV1::Attack))
    {
        return Ok(());
    }
    let target = if side == DiagnosticAffectedSideV1::Player {
        origin
    } else {
        origin.other()
    };
    let amount = effect_amount(
        origin,
        value,
        multiplier,
        source.support_count,
        source.anti_support_count,
        rounds_played,
        opponent_stars,
        opponent_damage,
    )?;
    attack[target] = apply_u32_modifier(
        origin,
        attack[target],
        operation,
        amount,
        minimum.map(u32::from),
        maximum.map(u32::from),
    )?;
    Ok(())
}

fn effect_amount(
    player: PlayerId,
    value: u16,
    multiplier: DiagnosticMagnitudeV1,
    support_count: u16,
    anti_support_count: u16,
    rounds_played: u8,
    opponent_stars: u16,
    // The opposing card's Damage as the Attack phase sees it: resolved, but before Fury.
    // Zero on the Power/Damage path, which never applies an Attack effect.
    opponent_damage: u16,
) -> Result<u32, CombatResolutionError> {
    let multiplier = match multiplier {
        DiagnosticMagnitudeV1::Fixed => 1,
        DiagnosticMagnitudeV1::SourceBonusSupport => u32::from(support_count),
        DiagnosticMagnitudeV1::AntiSupport => u32::from(anti_support_count),
        DiagnosticMagnitudeV1::Growth => u32::from(rounds_played) + 1,
        DiagnosticMagnitudeV1::Degrowth => u32::from(MAX_ROUNDS.checked_sub(rounds_played).ok_or(
            CombatResolutionError {
                player,
                stage: CombatResolutionArithmeticStage::EffectMagnitude,
            },
        )?),
        DiagnosticMagnitudeV1::OpponentStars => u32::from(opponent_stars),
        DiagnosticMagnitudeV1::OpponentDamage => u32::from(opponent_damage),
    };
    u32::from(value)
        .checked_mul(multiplier)
        .ok_or(CombatResolutionError {
            player,
            stage: CombatResolutionArithmeticStage::EffectMagnitude,
        })
}

fn apply_u16_modifier(
    player: PlayerId,
    stage: CombatResolutionArithmeticStage,
    current: u16,
    operation: DiagnosticStatOperationV1,
    amount: u32,
    minimum: Option<u16>,
    maximum: Option<u16>,
) -> Result<u16, CombatResolutionError> {
    let current = u32::from(current);
    let next = match operation {
        DiagnosticStatOperationV1::Increase => match maximum.map(u32::from) {
            Some(maximum) if current < maximum => current
                .checked_add(amount)
                .ok_or(CombatResolutionError { player, stage })?
                .min(maximum),
            Some(_) => current,
            None => current
                .checked_add(amount)
                .ok_or(CombatResolutionError { player, stage })?,
        },
        DiagnosticStatOperationV1::Decrease => match minimum.map(u32::from) {
            Some(minimum) if current > minimum => current.saturating_sub(amount).max(minimum),
            Some(_) => current,
            None => current.saturating_sub(amount),
        },
    };
    u16::try_from(next).map_err(|_| CombatResolutionError { player, stage })
}

fn apply_u32_modifier(
    player: PlayerId,
    current: u32,
    operation: DiagnosticStatOperationV1,
    amount: u32,
    minimum: Option<u32>,
    maximum: Option<u32>,
) -> Result<u32, CombatResolutionError> {
    match operation {
        DiagnosticStatOperationV1::Increase => match maximum {
            Some(maximum) if current < maximum => current
                .checked_add(amount)
                .ok_or(CombatResolutionError {
                    player,
                    stage: CombatResolutionArithmeticStage::Attack,
                })
                .map(|value| value.min(maximum)),
            Some(_) => Ok(current),
            None => current.checked_add(amount).ok_or(CombatResolutionError {
                player,
                stage: CombatResolutionArithmeticStage::Attack,
            }),
        },
        DiagnosticStatOperationV1::Decrease => Ok(match minimum {
            Some(minimum) if current > minimum => current.saturating_sub(amount).max(minimum),
            Some(_) => current,
            None => current.saturating_sub(amount),
        }),
    }
}
