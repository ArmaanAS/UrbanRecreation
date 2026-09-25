//! Catalog-only construction for fully executable combat-stat matches.
//!
//! This is deliberately separate from replay preparation. It derives immutable whole-draw
//! context from canonical cards, resolves printed descriptions through the reviewed effect
//! registry, and refuses a match if any legal card could reach an unsupported effect.

use super::combat_stat_compiler::{
    classify_anita_courage_damage_to_life, classify_argos_defeat_capped_pillz,
    classify_backlash_life, classify_bet_gated_post_round, classify_both_players_life_reduction,
    classify_brawl_post_round, classify_clan_gated_post_round, classify_combat_stat_effect,
    classify_combust_opponent_life_and_pillz_on_victory, classify_conditional_control,
    classify_conditional_stat_copy, classify_conditional_stop,
    classify_consume_opponent_pillz_on_victory, classify_copy_opponent_source,
    classify_corrupt_own_life, classify_defeat_capped_life, classify_defeat_life,
    classify_defeat_opponent_life, classify_defeat_opponent_pillz,
    classify_defeat_opponent_pillz_gain, classify_defeat_pillz, classify_defeat_pillz_and_life,
    classify_dope_pillz, classify_equalizer_opponent_life_on_victory,
    classify_equalizer_post_round_gain, classify_growth_permanent,
    classify_hand_clan_gated_post_round, classify_heal_life_on_victory,
    classify_killshot_opponent_life, classify_killshot_pillz_and_life,
    classify_killshot_post_round, classify_komboka_victory_pillz_and_life,
    classify_poison_opponent_life_on_defeat, classify_poison_opponent_life_on_victory,
    classify_reanimate_life, classify_recover_pillz, classify_regen_life_on_victory,
    classify_round_scaled_post_round, classify_stop_triggered_opponent_pillz,
    classify_support_post_round, classify_toxin_opponent_life_on_victory,
    classify_unison_defeat_life, classify_unison_pillz_and_life, classify_victory_life,
    classify_victory_life_per_damage, classify_victory_life_per_opponent_damage,
    classify_victory_opponent_life, classify_victory_opponent_pillz,
    classify_victory_opponent_pillz_and_life, classify_victory_or_defeat_both_players_gain,
    classify_victory_or_defeat_life, classify_victory_or_defeat_life_per_damage,
    classify_victory_or_defeat_pillz, classify_victory_or_defeat_pillz_amount,
    classify_victory_pillz, classify_victory_pillz_max, classify_victory_pillz_per_damage,
    compact_effect, is_copy_opponent_source_description, BothPlayersGainV1,
    VictoryOrDefeatLifeEffectV1, COMBAT_STAT_COMPILER_POLICY_SEMANTIC_REVISION_V1,
};
use super::effect_reads_support_count;
use super::CopiedSourceKindV1;
use super::{
    BaseRulesCardSpec, BaseRulesMatchSpec, BaseRulesPlayerSpec, ByPlayer, CombatStatCardPlanV1,
    CombatStatDiagnosticMatchSpecV1, CombatStatDiagnosticV1, CombatStatEffectSourceV1,
    CombatStatEffectV1, CombatStatPlanErrorV1, CombatStatPostRoundEffectV1, CombatStatPredicateV1,
    CombatStatSourcePlanV1, HandSlot, PlayerId, HAND_SIZE,
};
use crate::catalog::{
    CanonicalCard, CardCatalog, CardKey, EffectiveCardCatalog,
    EffectiveCatalogSourceFingerprintFnv1a64,
};
use crate::effect_registry::{
    EffectDefinitionV1, EffectLookupError, EffectRegistryV1, SourceFingerprintFnv1a64,
    SupportedEffectV1, UnsupportedReasonV1,
};
use std::error::Error;
use std::fmt;

pub const LEADER_CLAN_ID: u32 = 36;
pub const OCULUS_CLAN_ID: u32 = 56;
const VORTEX_CLAN_ID: u32 = 45;
const VORTEX_CATALOG_BONUS_ID: u32 = 43;
const DEFEAT_RECOVER_DESCRIPTION: &str = "Defeat: Recover 2 Pillz Out Of 3";
const ANITA_COURAGE_DAMAGE_TO_LIFE_DESCRIPTION: &str = "Courage: +1 Life Per Dmg";
const ANITA_COURAGE_DAMAGE_TO_LIFE_REGISTRY_ID: u32 = 274;
const ARGOS_DEFEAT_CAPPED_PILLZ_DESCRIPTION: &str = "Defeat: +2 Pillz Max. 11";
const ARGOS_DEFEAT_CAPPED_PILLZ_REGISTRY_ID: u32 = 1158;
const LOBO_REANIMATE_REGISTRY_ID: u32 = 4951;
const BERZERK_CLAN_ID: u32 = 46;
const BERZERK_CATALOG_BONUS_ID: u32 = 44;
const BERZERK_VICTORY_OPPONENT_LIFE_REGISTRY_ID: u32 = 680;
const BERZERK_VICTORY_OPPONENT_LIFE_DESCRIPTION: &str = "-2 Opp. Life Min 2";
/// Printed card abilities that carry an identity-locked *conditional* Victory opponent-Life
/// reduction, keyed by the exact card and its printed catalog id, which must equal the
/// registry definition id. The catalog is authority here: description equality never
/// transfers one of these to another card, and a level whose printed id has no registry
/// definition — Doela Noel level one's `4843` — stays fail-closed with no special handling.
/// The unconditional printed abilities are admitted by grammar instead, under the ordinary
/// structural-alias rule every other post-round grammar uses.
const VICTORY_OPPONENT_LIFE_ABILITY_CARDS: [(CardKey, u32, &str); 7] = [
    (
        CardKey { id: 2058, level: 2 },
        4708,
        "Symmetry: - 4 Opp. Life Min 0",
    ),
    (
        CardKey { id: 2270, level: 1 },
        4301,
        "Confidence: -3 Opp. Life, Min 0",
    ),
    (
        CardKey { id: 2270, level: 2 },
        3016,
        "Confidence: -3 Opp. Life, Min 0",
    ),
    // Dragomer Cr level 3. Levels 4 and 5 print `3001` and `2302`, neither of which has a
    // registry definition, so they stay fail-closed by simply not being listed here.
    (
        CardKey { id: 2114, level: 3 },
        3314,
        "Courage: - 1 Opp. Life Min 0",
    ),
    // Ligea at all three levels. Levels 1 and 2 print byte-identical text under two ids, so
    // the card and level are what separate them, exactly as for Diabolus above.
    (
        CardKey { id: 2556, level: 1 },
        4531,
        "Courage: - 3 Opp. Life Min 1",
    ),
    (
        CardKey { id: 2556, level: 2 },
        4532,
        "Courage: - 3 Opp. Life Min 1",
    ),
    (
        CardKey { id: 2556, level: 3 },
        4533,
        "Courage: - 3 Opp. Life Min 0",
    ),
];
const RIOTS_CLAN_ID: u32 = 49;
const RIOTS_CATALOG_BONUS_ID: u32 = 47;
const VICTORY_OR_DEFEAT_PILLZ_DESCRIPTION: &str = "Victory Or Defeat : +1 Pillz";
const VICTORY_OR_DEFEAT_RIOTS_BONUS_REGISTRY_ID: u32 = 1034;
const VICTORY_OR_DEFEAT_GAIN_LIFE_ONE_DESCRIPTION: &str = "Victory Or Defeat : +1 Life";
const VICTORY_OR_DEFEAT_GAIN_LIFE_TWO_DESCRIPTION: &str = "Victory Or Defeat : +2 Life";
const VICTORY_OR_DEFEAT_REDUCE_OPPONENT_LIFE_DESCRIPTION: &str =
    "Victory Or Defeat: - 1 Opp. Life Min 1";
const EQUALIZER_REDUCE_OPPONENT_LIFE_DESCRIPTION: &str = "Equalizer: - 1 Opp. Life Min 2";
const KOMBOKA_CLAN_ID: u32 = 54;
const KOMBOKA_CATALOG_BONUS_ID: u32 = 53;
const KOMBOKA_VICTORY_PILLZ_AND_LIFE_DESCRIPTION: &str = "+1 Pillz And Life";
const KOMBOKA_VICTORY_PILLZ_AND_LIFE_REGISTRY_ID: u32 = 1714;
const JUNGO_CLAN_ID: u32 = 43;
/// The Zenith clan bonus `Bet > 3 Pillz: +3 Life` is catalog bonus 58 and capture-registry
/// definition 4657; like Jungo's it is bridged only for its active effective clan.
const ZENITH_CLAN_ID: u32 = 59;
const ZENITH_CATALOG_BONUS_ID: u32 = 58;
const JUNGO_CATALOG_BONUS_ID: u32 = 41;
const JUNGO_VICTORY_LIFE_BONUS_REGISTRY_ID: u32 = 401;
const JUNGO_VICTORY_LIFE_DESCRIPTION: &str = "+2 Life";
const FREAKS_CLAN_ID: u32 = 40;
const FREAKS_CATALOG_BONUS_ID: u32 = 38;
const FREAKS_POISON_BONUS_REGISTRY_ID: u32 = 206;
const FREAKS_POISON_DESCRIPTION: &str = "Poison 2, Min 3";
const ROOTS_CLAN_ID: u32 = 29;
const ROOTS_CATALOG_BONUS_ID: u32 = 28;
const ROOTS_STOP_ABILITY_BONUS_REGISTRY_ID: u32 = 41;
const GHEIST_CLAN_ID: u32 = 32;
const GHEIST_CATALOG_BONUS_ID: u32 = 32;
const GHEIST_STOP_ABILITY_BONUS_REGISTRY_ID: u32 = 94;
const STOP_OPPONENT_ABILITY_DESCRIPTION: &str = "Stop Opp. Ability";
const REPRISAL_STOP_OPPONENT_ABILITY_DESCRIPTION: &str = "Reprisal: Stop Opp. Ability";
const PIRANAS_CLAN_ID: u32 = 42;
const PIRANAS_CATALOG_BONUS_ID: u32 = 40;
const PIRANAS_STOP_BONUS_REGISTRY_ID: u32 = 333;
const STOP_OPPONENT_BONUS_DESCRIPTION: &str = "Stop Opp. Bonus";
/// The Oblivion clan bonus `Copy: Opp. Ability` is catalog bonus 56 - registry definition 56
/// is `-4 Opp Damage, Min 2` - and capture-registry definition 2918, the id every captured
/// static block carries for it.
const OBLIVION_CLAN_ID: u32 = 57;
const OBLIVION_CATALOG_BONUS_ID: u32 = 56;
const OBLIVION_COPY_ABILITY_BONUS_REGISTRY_ID: u32 = 2918;
const COPY_OPPONENT_ABILITY_DESCRIPTION: &str = "Copy: Opp. Ability";
/// Revision 5 (compiler revision 69) lets a selected night variant, which the catalog gives
/// no numeric identity, reach the post-round grammars that print a `Night:` form - by its
/// exact text in a night match only (`require_catalog_alias_or_night_variant`).
/// Revision 6 (compiler revision 75) lets `+1 Pillz And Life` execute from a card's Ability
/// slot where the printed ability id is a structural alias of its definition (Carnibox L2's
/// `3356`); the Komboka clan requirement stays on the Bonus slot alone.
/// Revision 7 (compiler revision 76) sends a conditional Stop or conditional stat Copy with
/// no catalog id through the night-variant bridge's full conjunction instead of admitting it
/// on the missing id alone.
pub const CATALOG_CONTEXT_POLICY_SEMANTIC_REVISION_V1: u16 = 7;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CatalogCombatStatPlayerInputV1 {
    pub initial_life: u16,
    pub initial_pillz: u16,
    pub hand: [CardKey; HAND_SIZE],
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CatalogCombatStatMatchInputV1 {
    pub battle_rule_id: u32,
    pub night: bool,
    pub players: ByPlayer<CatalogCombatStatPlayerInputV1>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CatalogCombatStatProjectionV1 {
    RequireFullyExecutableDraws,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CatalogCombatStatModelV1 {
    CombatStatDiagnosticV1,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CatalogCombatStatProvenanceV1 {
    pub model: CatalogCombatStatModelV1,
    pub projection: CatalogCombatStatProjectionV1,
    pub effect_registry_schema_version: u16,
    pub effect_registry_source_fingerprint_fnv1a64: SourceFingerprintFnv1a64,
    pub effective_catalog_source_fingerprint_fnv1a64: EffectiveCatalogSourceFingerprintFnv1a64,
    pub compiler_policy_semantic_revision: u16,
    pub catalog_context_policy_semantic_revision: u16,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct EffectiveCatalogCardV1 {
    pub key: CardKey,
    pub canonical_clan_id: u32,
    pub effective_clan_id: u32,
    /// The clan whose printed bonus is active for this card. `None` covers singleton
    /// bonuses, Leader, and Oculus draws where infiltration does not apply.
    pub active_bonus_clan_id: Option<u32>,
    /// Distinct canonical character ids sharing this card's effective clan across the
    /// immutable whole draw, including this card even when no bonus is active.
    pub effective_clan_character_count: u16,
    pub source_bonus_support_count: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogPrintedModifierV1 {
    /// Numeric identity from the catalog when the selected day/night variant supplies one.
    pub catalog_id: Option<u32>,
    pub description: String,
}

/// Pure catalog derivation before the bounded execution policy is applied.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedCatalogCardV1 {
    pub effective: EffectiveCatalogCardV1,
    pub ability: Option<CatalogPrintedModifierV1>,
    pub bonus: Option<CatalogPrintedModifierV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogCombatStatModifierIdentityV1 {
    pub catalog_id: Option<u32>,
    pub description: String,
    pub registry_definition_id: u32,
    pub registry_alias_ids: Box<[u32]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CatalogCombatStatSourceDispositionV1 {
    Absent,
    Execute {
        identity: CatalogCombatStatModifierIdentityV1,
        effect: SupportedEffectV1,
        predicate: CombatStatPredicateV1,
    },
    /// The card is executable, but its effect is applied only after round damage has
    /// resolved. Its compact plan still carries `Always` so the engine can construct the
    /// post-round work without widening ordinary stat-effect classification.
    ExecutePostRound {
        identity: CatalogCombatStatModifierIdentityV1,
        effect: CombatStatPostRoundEffectV1,
        /// The condition the effect's own printed text names, or `Always`. A post-round
        /// plan may be conditional, so preparation metadata has to say so: without this a
        /// reviewed conditional reduction would read exactly like an unconditional one.
        predicate: CombatStatPredicateV1,
    },
    /// The card carries an unconditional Copy. It has no effect of its own: at round time
    /// it adopts the opposing selected card's named source, keeping this slot's kind for
    /// Stop liveness and this card's own Support context.
    CopyOpponentSource {
        identity: CatalogCombatStatModifierIdentityV1,
        copied: CopiedSourceKindV1,
        /// `Always` for an unconditional Copy; otherwise the condition the printed text
        /// names, which gates the adoption itself rather than the adopted effect.
        predicate: CombatStatPredicateV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogCombatStatCardPreparationV1 {
    pub key: CardKey,
    pub canonical_clan_id: u32,
    pub effective_clan_id: u32,
    pub effective_clan_character_count: u16,
    pub source_bonus_support_count: u16,
    pub source_ability_support_count: u16,
    pub ability: CatalogCombatStatSourceDispositionV1,
    pub bonus: CatalogCombatStatSourceDispositionV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogCombatStatMatchV1 {
    input: CatalogCombatStatMatchInputV1,
    match_spec: CombatStatDiagnosticMatchSpecV1,
    cards: ByPlayer<[CatalogCombatStatCardPreparationV1; HAND_SIZE]>,
    provenance: CatalogCombatStatProvenanceV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectiveCatalogHandErrorV1 {
    MissingCard {
        hand_slot: HandSlot,
        key: CardKey,
    },
    MissingClan {
        hand_slot: HandSlot,
        key: CardKey,
        clan_id: u32,
    },
}

impl fmt::Display for EffectiveCatalogHandErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCard { hand_slot, key } => write!(
                formatter,
                "slot {} references unknown card id {} level {}",
                hand_slot.get(),
                key.id,
                key.level
            ),
            Self::MissingClan {
                hand_slot,
                key,
                clan_id,
            } => write!(
                formatter,
                "slot {} card id {} level {} references missing clan {clan_id}",
                hand_slot.get(),
                key.id,
                key.level
            ),
        }
    }
}

impl Error for EffectiveCatalogHandErrorV1 {}

#[derive(Debug)]
pub enum CatalogCombatStatMatchErrorV1 {
    Hand {
        player: PlayerId,
        source: EffectiveCatalogHandErrorV1,
    },
    DuplicateCharacter {
        player: PlayerId,
        first_slot: HandSlot,
        second_slot: HandSlot,
        character_id: u32,
    },
    WholeHandLeaderHazard {
        player: PlayerId,
        hand_slot: HandSlot,
        key: CardKey,
        name: String,
    },
    Lookup {
        player: PlayerId,
        hand_slot: HandSlot,
        source_kind: CombatStatEffectSourceV1,
        catalog_id: Option<u32>,
        description: String,
        source: EffectLookupError,
    },
    UnsupportedSource {
        player: PlayerId,
        hand_slot: HandSlot,
        source_kind: CombatStatEffectSourceV1,
        catalog_id: Option<u32>,
        description: String,
        registry_definition_id: u32,
        registry_reasons: Box<[UnsupportedReasonV1]>,
    },
    UnsupportedCompiledShape {
        player: PlayerId,
        hand_slot: HandSlot,
        source_kind: CombatStatEffectSourceV1,
        catalog_id: Option<u32>,
        registry_definition_id: u32,
    },
    EnginePlan(CombatStatPlanErrorV1),
}

impl fmt::Display for CatalogCombatStatMatchErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hand { player, source } => write!(formatter, "{player:?} {source}"),
            Self::DuplicateCharacter {
                player,
                first_slot,
                second_slot,
                character_id,
            } => write!(
                formatter,
                "{player:?} slots {} and {} repeat character id {character_id}",
                first_slot.get(),
                second_slot.get()
            ),
            Self::WholeHandLeaderHazard {
                player,
                hand_slot,
                key,
                name,
            } => write!(
                formatter,
                "{player:?} slot {} contains unsupported Leader {name} (id {} level {})",
                hand_slot.get(),
                key.id,
                key.level
            ),
            Self::Lookup {
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                source,
            } => write!(
                formatter,
                "{player:?} slot {} {source_kind:?} catalog source {catalog_id:?} {description:?} lookup failed: {source}",
                hand_slot.get()
            ),
            Self::UnsupportedSource {
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                registry_definition_id,
                ..
            } => write!(
                formatter,
                "{player:?} slot {} {source_kind:?} catalog source {catalog_id:?} {description:?} is not executable by this projection (registry definition {registry_definition_id})",
                hand_slot.get()
            ),
            Self::UnsupportedCompiledShape {
                player,
                hand_slot,
                source_kind,
                catalog_id,
                registry_definition_id,
            } => write!(
                formatter,
                "{player:?} slot {} {source_kind:?} catalog source {catalog_id:?} registry definition {registry_definition_id} cannot map to a compact plan",
                hand_slot.get()
            ),
            Self::EnginePlan(source) => write!(formatter, "catalog match plan is invalid: {source}"),
        }
    }
}

impl Error for CatalogCombatStatMatchErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Hand { source, .. } => Some(source),
            Self::Lookup { source, .. } => Some(source),
            Self::EnginePlan(source) => Some(source),
            Self::DuplicateCharacter { .. }
            | Self::WholeHandLeaderHazard { .. }
            | Self::UnsupportedSource { .. }
            | Self::UnsupportedCompiledShape { .. } => None,
        }
    }
}

struct PreparedCatalogSourceV1 {
    metadata: CatalogCombatStatSourceDispositionV1,
    compact: CombatStatSourcePlanV1,
}

impl CatalogCombatStatMatchV1 {
    pub fn new(
        input: CatalogCombatStatMatchInputV1,
        catalog: &EffectiveCardCatalog,
        registry: &EffectRegistryV1,
        projection: CatalogCombatStatProjectionV1,
    ) -> Result<Self, CatalogCombatStatMatchErrorV1> {
        let mut base_players: ByPlayer<Option<BaseRulesPlayerSpec>> = ByPlayer::new(None, None);
        let mut compact_cards: ByPlayer<[Option<CombatStatCardPlanV1>; HAND_SIZE]> =
            ByPlayer::new([const { None }; HAND_SIZE], [const { None }; HAND_SIZE]);
        let mut metadata: ByPlayer<[Option<CatalogCombatStatCardPreparationV1>; HAND_SIZE]> =
            ByPlayer::new([const { None }; HAND_SIZE], [const { None }; HAND_SIZE]);

        for player in PlayerId::ALL {
            validate_solver_hand(player, input.players[player].hand, catalog)?;
            let derived = derive_catalog_hand(input.players[player].hand, input.night, catalog)
                .map_err(|source| CatalogCombatStatMatchErrorV1::Hand { player, source })?;
            let mut base_hand: [Option<BaseRulesCardSpec>; HAND_SIZE] = [const { None }; HAND_SIZE];
            for slot in HandSlot::ALL {
                let index = slot.index();
                let derived = &derived[index];
                let effective = derived.effective;
                let card = catalog
                    .get(effective.key)
                    .expect("effective-hand derivation validated every card key");
                base_hand[index] = Some(BaseRulesCardSpec {
                    key: card.key(),
                    clan_id: card.clan_id,
                    power: u16::from(card.power),
                    damage: u16::from(card.damage),
                });
                let ability = if let Some(source) = &derived.ability {
                    prepare_catalog_source(
                        registry,
                        player,
                        slot,
                        card.key(),
                        CombatStatEffectSourceV1::Ability,
                        effective.effective_clan_id,
                        input.night,
                        source.catalog_id,
                        &source.description,
                    )?
                } else {
                    absent_source()
                };
                let bonus = if let Some(source) = &derived.bonus {
                    prepare_catalog_source(
                        registry,
                        player,
                        slot,
                        card.key(),
                        CombatStatEffectSourceV1::Bonus,
                        effective.effective_clan_id,
                        input.night,
                        source.catalog_id,
                        &source.description,
                    )?
                } else {
                    absent_source()
                };
                compact_cards[player][index] = Some(CombatStatCardPlanV1 {
                    key: card.key(),
                    effective_clan_id: effective.effective_clan_id,
                    ability: ability.compact,
                    bonus: bonus.compact,
                    source_bonus_support_count: effective.source_bonus_support_count,
                    source_ability_support_count: executable_ability_support_count(
                        ability.compact,
                        effective.effective_clan_character_count,
                    ),
                });
                metadata[player][index] = Some(CatalogCombatStatCardPreparationV1 {
                    key: card.key(),
                    canonical_clan_id: effective.canonical_clan_id,
                    effective_clan_id: effective.effective_clan_id,
                    effective_clan_character_count: effective.effective_clan_character_count,
                    source_bonus_support_count: effective.source_bonus_support_count,
                    source_ability_support_count: executable_ability_support_count(
                        ability.compact,
                        effective.effective_clan_character_count,
                    ),
                    ability: ability.metadata,
                    bonus: bonus.metadata,
                });
            }
            base_players[player] = Some(BaseRulesPlayerSpec {
                initial_life: input.players[player].initial_life,
                initial_pillz: input.players[player].initial_pillz,
                hand: base_hand.map(|card| card.expect("all four base cards were prepared")),
            });
        }

        // Some sources are admitted only where their context is one the corpus has pinned:
        // a `Stop:` source where nothing opposite can stop its owner's ability, a resource
        // canceller where nothing opposite has an effect whose cancellation is unpinned, a
        // `/ Life Lost` magnitude where nothing can raise its owner's Life, `Tune Out` where
        // no Killshot and no opposing Power/Attack cancel can meet it.
        // The engine refuses such a plan too; refusing it here, as the unsupported source it
        // is, keeps the coverage report counting it as a blocker.
        for player in PlayerId::ALL {
            let opponent = compact_cards[player.other()]
                .each_ref()
                .map(|card| card.expect("all eight compact cards were prepared"));
            let own = compact_cards[player]
                .each_ref()
                .map(|card| card.expect("all eight compact cards were prepared"));
            for slot in HandSlot::ALL {
                let compact = compact_cards[player][slot.index()]
                    .expect("all eight compact cards were prepared");
                let card = metadata[player][slot.index()]
                    .as_ref()
                    .expect("all eight metadata cards were prepared");
                for (source_kind, plan, disposition) in [
                    (
                        CombatStatEffectSourceV1::Ability,
                        compact.ability,
                        &card.ability,
                    ),
                    (CombatStatEffectSourceV1::Bonus, compact.bonus, &card.bonus),
                ] {
                    if super::combat_stat_diagnostic::unmodelled_source_context(
                        plan, slot, &own, &opponent,
                    )
                    .is_none()
                    {
                        continue;
                    }
                    let (CatalogCombatStatSourceDispositionV1::Execute { identity, .. }
                    | CatalogCombatStatSourceDispositionV1::ExecutePostRound {
                        identity, ..
                    }
                    | CatalogCombatStatSourceDispositionV1::CopyOpponentSource {
                        identity, ..
                    }) = disposition
                    else {
                        continue;
                    };
                    return Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
                        player,
                        hand_slot: slot,
                        source_kind,
                        catalog_id: identity.catalog_id,
                        description: identity.description.clone(),
                        registry_definition_id: identity.registry_definition_id,
                        registry_reasons: Box::new([]),
                    });
                }
            }
        }
        let match_spec = CombatStatDiagnosticMatchSpecV1 {
            base_rules: BaseRulesMatchSpec {
                battle_rule_id: input.battle_rule_id,
                night: input.night,
                players: base_players.map(|player| player.expect("both players were prepared")),
            },
            cards: compact_cards
                .map(|hand| hand.map(|card| card.expect("all eight compact cards were prepared"))),
        };
        // `After` and `Versus` read canonical clans; where an infiltrating Oculus in the hand
        // the gate reads would make the canonical and effective readings disagree the gate is
        // unpinned, and it is refused as the unsupported source it is so the coverage report
        // still counts it.
        for player in PlayerId::ALL {
            for slot in HandSlot::ALL {
                let compact = match_spec.cards[player][slot.index()];
                let card = metadata[player][slot.index()]
                    .as_ref()
                    .expect("all eight metadata cards were prepared");
                for (source_kind, plan, disposition) in [
                    (
                        CombatStatEffectSourceV1::Ability,
                        compact.ability,
                        &card.ability,
                    ),
                    (CombatStatEffectSourceV1::Bonus, compact.bonus, &card.bonus),
                ] {
                    let CombatStatSourcePlanV1::Execute { predicate, .. } = plan else {
                        continue;
                    };
                    if !super::combat_stat_diagnostic::clan_gate_is_ambiguous(
                        player,
                        source_kind,
                        predicate,
                        &match_spec.base_rules,
                        &match_spec.cards,
                    ) {
                        continue;
                    }
                    let (CatalogCombatStatSourceDispositionV1::Execute { identity, .. }
                    | CatalogCombatStatSourceDispositionV1::ExecutePostRound {
                        identity, ..
                    }
                    | CatalogCombatStatSourceDispositionV1::CopyOpponentSource {
                        identity, ..
                    }) = disposition
                    else {
                        continue;
                    };
                    return Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
                        player,
                        hand_slot: slot,
                        source_kind,
                        catalog_id: identity.catalog_id,
                        description: identity.description.clone(),
                        registry_definition_id: identity.registry_definition_id,
                        registry_reasons: Box::new([]),
                    });
                }
            }
        }
        CombatStatDiagnosticV1::new(match_spec.clone())
            .map_err(CatalogCombatStatMatchErrorV1::EnginePlan)?;
        Ok(Self {
            input,
            match_spec,
            cards: metadata
                .map(|hand| hand.map(|card| card.expect("all eight metadata cards were prepared"))),
            provenance: CatalogCombatStatProvenanceV1 {
                model: CatalogCombatStatModelV1::CombatStatDiagnosticV1,
                projection,
                effect_registry_schema_version: registry.schema_version(),
                effect_registry_source_fingerprint_fnv1a64: registry.source_fingerprint_fnv1a64(),
                effective_catalog_source_fingerprint_fnv1a64: catalog.source_fingerprint_fnv1a64(),
                compiler_policy_semantic_revision: COMBAT_STAT_COMPILER_POLICY_SEMANTIC_REVISION_V1,
                catalog_context_policy_semantic_revision:
                    CATALOG_CONTEXT_POLICY_SEMANTIC_REVISION_V1,
            },
        })
    }

    pub fn input(&self) -> &CatalogCombatStatMatchInputV1 {
        &self.input
    }

    pub fn match_spec(&self) -> &CombatStatDiagnosticMatchSpecV1 {
        &self.match_spec
    }

    pub fn preparation(&self) -> &ByPlayer<[CatalogCombatStatCardPreparationV1; HAND_SIZE]> {
        &self.cards
    }

    pub const fn provenance(&self) -> CatalogCombatStatProvenanceV1 {
        self.provenance
    }

    pub fn new_game(&self) -> CombatStatDiagnosticV1 {
        CombatStatDiagnosticV1::new(self.match_spec.clone())
            .expect("catalog match plan was validated at construction")
    }
}

/// Selects the exact printed day/night ability and derived active bonus without compiling
/// either source. The returned descriptions are suitable for corpus comparison and strict
/// registry resolution; no capture-resolved Copy result enters this layer.
pub fn derive_catalog_hand(
    keys: [CardKey; HAND_SIZE],
    night: bool,
    catalog: &EffectiveCardCatalog,
) -> Result<[DerivedCatalogCardV1; HAND_SIZE], EffectiveCatalogHandErrorV1> {
    let effective = derive_effective_catalog_hand(keys, catalog.as_catalog())?;
    let mut derived: [Option<DerivedCatalogCardV1>; HAND_SIZE] = [const { None }; HAND_SIZE];
    for slot in HandSlot::ALL {
        let index = slot.index();
        let card = catalog
            .get(keys[index])
            .expect("effective-hand derivation validated every card key");
        let uses_night_ability = night && card.night_ability.is_some();
        let ability_description = if uses_night_ability {
            card.night_ability.as_deref().unwrap_or(&card.ability)
        } else {
            &card.ability
        };
        let ability = (ability_description != "No Ability").then(|| CatalogPrintedModifierV1 {
            catalog_id: (!uses_night_ability && card.ability_id != 0).then_some(card.ability_id),
            description: ability_description.to_owned(),
        });
        let bonus = if let Some(clan_id) = effective[index].active_bonus_clan_id {
            let clan =
                catalog
                    .get_clan(clan_id)
                    .ok_or(EffectiveCatalogHandErrorV1::MissingClan {
                        hand_slot: slot,
                        key: card.key(),
                        clan_id,
                    })?;
            let uses_night_bonus = night && clan.night_bonus.is_some();
            let description = if uses_night_bonus {
                clan.night_bonus.as_deref().unwrap_or(&clan.bonus)
            } else {
                &clan.bonus
            };
            (description != "No Bonus").then(|| CatalogPrintedModifierV1 {
                catalog_id: (!uses_night_bonus && clan.bonus_id != 0).then_some(clan.bonus_id),
                description: description.to_owned(),
            })
        } else {
            None
        };
        derived[index] = Some(DerivedCatalogCardV1 {
            effective: effective[index],
            ability,
            bonus,
        });
    }
    Ok(derived.map(|card| card.expect("all four catalog sources were derived")))
}

/// Derives immutable effective-clan and bonus-activation context without consulting a
/// capture. This function intentionally classifies Leader rather than rejecting it; the
/// strict solver constructor rejects Leader separately, while corpus diagnostics can still
/// inspect the remaining cards in such hands.
pub fn derive_effective_catalog_hand(
    keys: [CardKey; HAND_SIZE],
    catalog: &CardCatalog,
) -> Result<[EffectiveCatalogCardV1; HAND_SIZE], EffectiveCatalogHandErrorV1> {
    let mut cards: [Option<&CanonicalCard>; HAND_SIZE] = [const { None }; HAND_SIZE];
    for slot in HandSlot::ALL {
        let key = keys[slot.index()];
        let card = catalog
            .get(key)
            .ok_or(EffectiveCatalogHandErrorV1::MissingCard {
                hand_slot: slot,
                key,
            })?;
        if catalog.get_clan(card.clan_id).is_none() {
            return Err(EffectiveCatalogHandErrorV1::MissingClan {
                hand_slot: slot,
                key,
                clan_id: card.clan_id,
            });
        }
        cards[slot.index()] = Some(card);
    }
    let cards = cards.map(|card| card.expect("all four catalog cards were resolved"));
    let oculus_slots: Vec<_> = HandSlot::ALL
        .into_iter()
        .filter(|slot| cards[slot.index()].clan_id == OCULUS_CLAN_ID)
        .collect();
    let infiltrated_clan = if oculus_slots.len() == 1 {
        let oculus_slot = oculus_slots[0];
        let mut clans = [0_u32; HAND_SIZE - 1];
        let mut counts = [0_u8; HAND_SIZE - 1];
        let mut clan_count = 0_usize;
        for slot in HandSlot::ALL {
            if slot == oculus_slot {
                continue;
            }
            let clan_id = cards[slot.index()].clan_id;
            if let Some(index) = clans[..clan_count].iter().position(|id| *id == clan_id) {
                counts[index] += 1;
            } else {
                clans[clan_count] = clan_id;
                counts[clan_count] = 1;
                clan_count += 1;
            }
        }
        match clan_count {
            1 => Some(clans[0]),
            2 => (0..clan_count)
                .find(|index| counts[*index] == 1)
                .map(|index| clans[index]),
            _ => None,
        }
    } else {
        None
    };

    let effective_clans: [u32; HAND_SIZE] = std::array::from_fn(|index| {
        if cards[index].clan_id == OCULUS_CLAN_ID && oculus_slots.len() == 1 {
            infiltrated_clan.unwrap_or(OCULUS_CLAN_ID)
        } else {
            cards[index].clan_id
        }
    });
    Ok(std::array::from_fn(|index| {
        let effective_clan_id = effective_clans[index];
        let effective_clan_character_count = cards
            .iter()
            .enumerate()
            .filter(|(other, _)| effective_clans[*other] == effective_clan_id)
            .fold(
                ([0_u32; HAND_SIZE], 0_usize),
                |(mut ids, mut count), (_, card)| {
                    if !ids[..count].contains(&card.id) {
                        ids[count] = card.id;
                        count += 1;
                    }
                    (ids, count)
                },
            )
            .1 as u16;
        let has_bonus_source = effective_clan_id != LEADER_CLAN_ID
            && !(cards[index].clan_id == OCULUS_CLAN_ID && infiltrated_clan.is_none());
        let active = has_bonus_source && effective_clan_character_count >= 2;
        EffectiveCatalogCardV1 {
            key: cards[index].key(),
            canonical_clan_id: cards[index].clan_id,
            effective_clan_id,
            active_bonus_clan_id: active.then_some(effective_clan_id),
            effective_clan_character_count,
            source_bonus_support_count: if active {
                effective_clan_character_count
            } else {
                0
            },
        }
    }))
}

fn executable_ability_support_count(
    plan: CombatStatSourcePlanV1,
    effective_clan_character_count: u16,
) -> u16 {
    // A Copy ability may adopt an opposing Support effect, and Support is then counted in
    // this card's own effective clan, so it always carries that context.
    match plan {
        CombatStatSourcePlanV1::Execute { effect, .. } if effect_reads_support_count(effect) => {
            effective_clan_character_count
        }
        CombatStatSourcePlanV1::CopyOpponentSource { .. } => effective_clan_character_count,
        _ => 0,
    }
}

fn validate_solver_hand(
    player: PlayerId,
    keys: [CardKey; HAND_SIZE],
    catalog: &EffectiveCardCatalog,
) -> Result<(), CatalogCombatStatMatchErrorV1> {
    for slot in HandSlot::ALL {
        let key = keys[slot.index()];
        let card = catalog
            .get(key)
            .ok_or(CatalogCombatStatMatchErrorV1::Hand {
                player,
                source: EffectiveCatalogHandErrorV1::MissingCard {
                    hand_slot: slot,
                    key,
                },
            })?;
        for prior in HandSlot::ALL.into_iter().take(slot.index()) {
            if keys[prior.index()].id == key.id {
                return Err(CatalogCombatStatMatchErrorV1::DuplicateCharacter {
                    player,
                    first_slot: prior,
                    second_slot: slot,
                    character_id: key.id,
                });
            }
        }
        if card.clan_id == LEADER_CLAN_ID {
            return Err(CatalogCombatStatMatchErrorV1::WholeHandLeaderHazard {
                player,
                hand_slot: slot,
                key,
                name: card.name.clone(),
            });
        }
    }
    Ok(())
}

fn absent_source() -> PreparedCatalogSourceV1 {
    PreparedCatalogSourceV1 {
        metadata: CatalogCombatStatSourceDispositionV1::Absent,
        compact: CombatStatSourcePlanV1::Absent,
    }
}

fn prepare_catalog_source(
    registry: &EffectRegistryV1,
    player: PlayerId,
    hand_slot: HandSlot,
    card_key: CardKey,
    source_kind: CombatStatEffectSourceV1,
    effective_clan_id: u32,
    night: bool,
    catalog_id: Option<u32>,
    description: &str,
) -> Result<PreparedCatalogSourceV1, CatalogCombatStatMatchErrorV1> {
    if matches!(description, "No Ability" | "No Bonus") {
        return Ok(absent_source());
    }
    // Copy, unconditional or Reprisal/Revenge. The registry holds many structurally
    // identical Copy definitions, so the printed catalog id must itself be one of them: an
    // id that is not a definition of this exact text and shape stays fail-closed, and
    // description alone never admits.
    if is_copy_opponent_source_description(description) {
        // Clan bonus ids are a separate namespace: bridge only the active effective Oblivion
        // bonus, like the Stop bridges below. Its identity keeps the catalog id.
        let registry_id = match (source_kind, effective_clan_id, catalog_id) {
            (
                CombatStatEffectSourceV1::Bonus,
                OBLIVION_CLAN_ID,
                Some(OBLIVION_CATALOG_BONUS_ID),
            ) if description == COPY_OPPONENT_ABILITY_DESCRIPTION => {
                Some(OBLIVION_COPY_ABILITY_BONUS_REGISTRY_ID)
            }
            _ => catalog_id,
        };
        let definition = registry_id
            .and_then(|id| registry.lookup_capture(id, description).ok())
            .filter(|definition| classify_copy_opponent_source(definition).is_some());
        let Some(definition) = definition else {
            let fallback = registry
                .lookup_description(description)
                .map_err(|source| CatalogCombatStatMatchErrorV1::Lookup {
                    player,
                    hand_slot,
                    source_kind,
                    catalog_id,
                    description: description.to_owned(),
                    source,
                })?
                .definition();
            return Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description: description.to_owned(),
                registry_definition_id: fallback.id(),
                registry_reasons: fallback
                    .compiled()
                    .unsupported_reasons()
                    .to_vec()
                    .into_boxed_slice(),
            });
        };
        let (copied, predicate) = classify_copy_opponent_source(definition).expect("checked above");
        let registry_alias_ids = registry
            .lookup_description(description)
            .map_err(|source| CatalogCombatStatMatchErrorV1::Lookup {
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description: description.to_owned(),
                source,
            })?
            .alias_ids()
            .to_vec()
            .into_boxed_slice();
        return Ok(PreparedCatalogSourceV1 {
            metadata: CatalogCombatStatSourceDispositionV1::CopyOpponentSource {
                identity: CatalogCombatStatModifierIdentityV1 {
                    catalog_id,
                    description: description.to_owned(),
                    registry_definition_id: definition.id(),
                    registry_alias_ids,
                },
                copied,
                predicate,
            },
            compact: CombatStatSourcePlanV1::CopyOpponentSource {
                source_id: definition.id(),
                copied,
                predicate,
            },
        });
    }
    if description == STOP_OPPONENT_ABILITY_DESCRIPTION {
        let registry_definition_id = match (source_kind, effective_clan_id, catalog_id) {
            // Static card abilities carry capture-registry identities in the catalog.
            (CombatStatEffectSourceV1::Ability, _, Some(id)) => Some(id),
            // Clan bonus ids are a separate namespace. Bridge only these exact active
            // effective-clan sources to independently captured unconditional SOA records.
            (CombatStatEffectSourceV1::Bonus, ROOTS_CLAN_ID, Some(ROOTS_CATALOG_BONUS_ID)) => {
                Some(ROOTS_STOP_ABILITY_BONUS_REGISTRY_ID)
            }
            (CombatStatEffectSourceV1::Bonus, GHEIST_CLAN_ID, Some(GHEIST_CATALOG_BONUS_ID)) => {
                Some(GHEIST_STOP_ABILITY_BONUS_REGISTRY_ID)
            }
            _ => None,
        };
        if let Some(registry_definition_id) = registry_definition_id {
            return prepare_control_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                registry_definition_id,
                SupportedEffectV1::StopOpponentAbility,
            );
        }
    }
    // Reprisal SOA is deliberately a separate, source-identity-gated slice.  The two
    // captured registry definitions share one structural alias group, but a catalog card
    // can enter only through its own printed id: description equality must not make every
    // Reprisal SOA card executable.
    if description == REPRISAL_STOP_OPPONENT_ABILITY_DESCRIPTION
        && source_kind == CombatStatEffectSourceV1::Ability
    {
        let match_ = registry.lookup_description(description).map_err(|source| {
            CatalogCombatStatMatchErrorV1::Lookup {
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description: description.to_owned(),
                source,
            }
        })?;
        if let Some(registry_definition_id) =
            catalog_id.filter(|catalog_id| match_.alias_ids().contains(catalog_id))
        {
            return prepare_control_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                registry_definition_id,
                SupportedEffectV1::StopOpponentAbility,
            );
        }
        let definition = match_.definition();
        return Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            player,
            hand_slot,
            source_kind,
            catalog_id,
            description: description.to_owned(),
            registry_definition_id: definition.id(),
            registry_reasons: definition
                .compiled()
                .unsupported_reasons()
                .to_vec()
                .into_boxed_slice(),
        });
    }
    // The Piranas catalog bonus id is also from a separate namespace. Pin its active
    // clan source to the independently captured registry identity used by real battles.
    if description == STOP_OPPONENT_BONUS_DESCRIPTION
        && source_kind == CombatStatEffectSourceV1::Bonus
        && effective_clan_id == PIRANAS_CLAN_ID
        && catalog_id == Some(PIRANAS_CATALOG_BONUS_ID)
    {
        return prepare_control_source(
            registry,
            player,
            hand_slot,
            source_kind,
            catalog_id,
            description,
            PIRANAS_STOP_BONUS_REGISTRY_ID,
            SupportedEffectV1::StopOpponentBonus,
        );
    }
    if let Some(registry_definition_id) = defeat_recover_registry_definition_id(
        source_kind,
        effective_clan_id,
        catalog_id,
        description,
    ) {
        return prepare_defeat_recover_source(
            registry,
            player,
            hand_slot,
            source_kind,
            catalog_id,
            description,
            registry_definition_id,
        );
    }
    // This conversion's runtime magnitude is the resolved damage, so it is not a
    // generic numeric modifier. It is admitted solely through Anita's printed level-3
    // Ability identity; description aliases, bonus provenance, and Copy have no
    // catalog authority.
    if description == ANITA_COURAGE_DAMAGE_TO_LIFE_DESCRIPTION {
        if anita_courage_damage_to_life_registry_definition_id(card_key, source_kind, catalog_id)
            .is_some()
        {
            return prepare_anita_courage_damage_to_life_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
            );
        }
        let definition = registry
            .lookup_description(description)
            .map_err(|source| CatalogCombatStatMatchErrorV1::Lookup {
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description: description.to_owned(),
                source,
            })?
            .definition();
        return Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            player,
            hand_slot,
            source_kind,
            catalog_id,
            description: description.to_owned(),
            registry_definition_id: definition.id(),
            registry_reasons: definition
                .compiled()
                .unsupported_reasons()
                .to_vec()
                .into_boxed_slice(),
        });
    }
    if description == ARGOS_DEFEAT_CAPPED_PILLZ_DESCRIPTION {
        if source_kind == CombatStatEffectSourceV1::Ability
            && catalog_id == Some(ARGOS_DEFEAT_CAPPED_PILLZ_REGISTRY_ID)
        {
            return prepare_argos_defeat_capped_pillz_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
            );
        }
        // Catalog execution is pinned to Argos' actual static ability id. Same text can
        // never inherit this identity, and dynamic Copy remains outside this constructor.
        let definition = registry
            .lookup_description(description)
            .map_err(|source| CatalogCombatStatMatchErrorV1::Lookup {
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description: description.to_owned(),
                source,
            })?
            .definition();
        return Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            player,
            hand_slot,
            source_kind,
            catalog_id,
            description: description.to_owned(),
            registry_definition_id: definition.id(),
            registry_reasons: definition
                .compiled()
                .unsupported_reasons()
                .to_vec()
                .into_boxed_slice(),
        });
    }
    // Victory opponent-Life has a closed set of reviewed catalog identities: three printed
    // card abilities and the active Berzerk clan Bonus. Same-text catalog ids (Rakhan 978,
    // Milovan 498, Fraser 1289) carry no registry definition, and description equality never
    // transfers this effect to another card or clan.
    if description == BERZERK_VICTORY_OPPONENT_LIFE_DESCRIPTION
        || VICTORY_OPPONENT_LIFE_ABILITY_CARDS
            .iter()
            .any(|(_, _, text)| *text == description)
    {
        if let Some(registry_definition_id) = victory_opponent_life_registry_definition_id(
            card_key,
            source_kind,
            effective_clan_id,
            catalog_id,
            description,
        ) {
            return prepare_victory_opponent_life_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                registry_definition_id,
            );
        }
        let definition = registry
            .lookup_description(description)
            .map_err(|source| CatalogCombatStatMatchErrorV1::Lookup {
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description: description.to_owned(),
                source,
            })?
            .definition();
        return Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            player,
            hand_slot,
            source_kind,
            catalog_id,
            description: description.to_owned(),
            registry_definition_id: definition.id(),
            registry_reasons: definition
                .compiled()
                .unsupported_reasons()
                .to_vec()
                .into_boxed_slice(),
        });
    }
    if description == VICTORY_OR_DEFEAT_PILLZ_DESCRIPTION {
        if let Some(registry_definition_id) = victory_or_defeat_pillz_registry_definition_id(
            source_kind,
            effective_clan_id,
            catalog_id,
        ) {
            return prepare_victory_or_defeat_pillz_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                registry_definition_id,
            );
        }
        // The registry intentionally groups several same-text identities. Catalog execution
        // must not inherit an executable identity from a description collision.
        let definition = registry
            .lookup_description(description)
            .map_err(|source| CatalogCombatStatMatchErrorV1::Lookup {
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description: description.to_owned(),
                source,
            })?
            .definition();
        return Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            player,
            hand_slot,
            source_kind,
            catalog_id,
            description: description.to_owned(),
            registry_definition_id: definition.id(),
            registry_reasons: definition
                .compiled()
                .unsupported_reasons()
                .to_vec()
                .into_boxed_slice(),
        });
    }
    // Victory Or Defeat Life is ability-only in canonical catalog data.  Captures can
    // materialize the reviewed registry identities through Copy under either source
    // kind, but a catalog-built hand must never synthesize that dynamic provenance.
    if victory_or_defeat_life_description(description) {
        if let Some(registry_definition_id) = victory_or_defeat_life_registry_definition_id(
            card_key,
            source_kind,
            catalog_id,
            description,
        ) {
            return prepare_victory_or_defeat_life_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                registry_definition_id,
            );
        }
        let definition = registry
            .lookup_description(description)
            .map_err(|source| CatalogCombatStatMatchErrorV1::Lookup {
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description: description.to_owned(),
                source,
            })?
            .definition();
        return Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            player,
            hand_slot,
            source_kind,
            catalog_id,
            description: description.to_owned(),
            registry_definition_id: definition.id(),
            registry_reasons: definition
                .compiled()
                .unsupported_reasons()
                .to_vec()
                .into_boxed_slice(),
        });
    }
    // The `Min 2` Equalizer opponent-Life text keeps its two reviewed canonical printed
    // abilities, by card key. Dynamic Copy is admitted by replay preparation, never by this
    // immutable catalog constructor. Every other printed Equalizer reduction is the grammar
    // below, under the ordinary catalog-alias rule.
    if description == EQUALIZER_REDUCE_OPPONENT_LIFE_DESCRIPTION {
        if let Some(registry_definition_id) =
            equalizer_opponent_life_registry_definition_id(card_key, source_kind, catalog_id)
        {
            return prepare_equalizer_opponent_life_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                registry_definition_id,
            );
        }
        let definition = registry
            .lookup_description(description)
            .map_err(|source| CatalogCombatStatMatchErrorV1::Lookup {
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description: description.to_owned(),
                source,
            })?
            .definition();
        return Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            player,
            hand_slot,
            source_kind,
            catalog_id,
            description: description.to_owned(),
            registry_definition_id: definition.id(),
            registry_reasons: definition
                .compiled()
                .unsupported_reasons()
                .to_vec()
                .into_boxed_slice(),
        });
    }
    // Komboka's clan bonus is a catalog namespace source, bridged only for the active
    // effective clan and its own catalog bonus id. Since revision 75 the same text also
    // executes from a card's Ability slot, but only where the printed ability id is itself a
    // structural alias of the definition - Carnibox L2's `3356`, which 1089974/1 shows paying
    // exactly as the bonus does. A same-text ability with no registry record of its own
    // (Chasey `1519`/`3648`, Van Dijk `3456`/`3457`) stays closed, and the ability alias never
    // lends authority to another clan's bonus.
    if description == KOMBOKA_VICTORY_PILLZ_AND_LIFE_DESCRIPTION {
        if source_kind == CombatStatEffectSourceV1::Bonus
            && effective_clan_id == KOMBOKA_CLAN_ID
            && catalog_id == Some(KOMBOKA_CATALOG_BONUS_ID)
        {
            return prepare_komboka_victory_pillz_and_life_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
            );
        }
        if source_kind == CombatStatEffectSourceV1::Ability {
            if let Ok(match_) = registry.lookup_description(description) {
                if catalog_id.is_some_and(|id| match_.alias_ids().contains(&id)) {
                    return prepare_komboka_victory_pillz_and_life_source(
                        registry,
                        player,
                        hand_slot,
                        source_kind,
                        catalog_id,
                        description,
                    );
                }
            }
        }
        let definition = registry
            .lookup_description(description)
            .map_err(|source| CatalogCombatStatMatchErrorV1::Lookup {
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description: description.to_owned(),
                source,
            })?
            .definition();
        return Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            player,
            hand_slot,
            source_kind,
            catalog_id,
            description: description.to_owned(),
            registry_definition_id: definition.id(),
            registry_reasons: definition
                .compiled()
                .unsupported_reasons()
                .to_vec()
                .into_boxed_slice(),
        });
    }
    // Jungo's catalog bonus id is not a capture-registry id. Bridge only its active
    // effective clan and exact printed source to the independently captured definition.
    if description == JUNGO_VICTORY_LIFE_DESCRIPTION
        && source_kind == CombatStatEffectSourceV1::Bonus
        && effective_clan_id == JUNGO_CLAN_ID
        && catalog_id == Some(JUNGO_CATALOG_BONUS_ID)
    {
        return prepare_victory_life_source(
            registry,
            player,
            hand_slot,
            source_kind,
            catalog_id,
            description,
            JUNGO_VICTORY_LIFE_BONUS_REGISTRY_ID,
        );
    }
    // Freaks' catalog bonus id is likewise not a capture-registry id. The same bridge:
    // active effective clan, Bonus slot and exact printed text, to the captured `206`.
    if description == FREAKS_POISON_DESCRIPTION
        && source_kind == CombatStatEffectSourceV1::Bonus
        && effective_clan_id == FREAKS_CLAN_ID
        && catalog_id == Some(FREAKS_CATALOG_BONUS_ID)
    {
        return prepare_permanent_life_source(
            registry,
            player,
            hand_slot,
            source_kind,
            catalog_id,
            description,
            FREAKS_POISON_BONUS_REGISTRY_ID,
        );
    }
    // Every other generic Victory Life source must carry an actual registry identity in
    // the catalog. Description equality alone is never authority to execute a Life effect.
    if let Ok(match_) = registry.lookup_description(description) {
        let definition = match_.definition();
        // A conditional Stop is admitted by grammar, so a printed card level must be a
        // structural alias of the definition its text resolves to rather than borrowing it.
        // A night variant has no catalog id at all; its text is the only identity it has,
        // as for the Night numerics - but since revision 7 only under the night-variant
        // bridge's full conjunction (a night match, `Night: ` text, `MatchIsNight`), never on
        // a missing catalog id alone.
        if let Some((_, predicate)) = classify_conditional_stop(definition, source_kind)
            .or_else(|| classify_conditional_stat_copy(definition, source_kind))
        {
            require_catalog_alias_or_night_variant(
                match_.alias_ids(),
                night,
                predicate,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
        }
        // Revision 75's conditional cancels and Reprisal Protection are admitted by text and
        // shape, so a printed level must likewise be a structural alias of the definition its
        // text resolves to. None prints a night variant.
        if classify_conditional_control(definition, source_kind).is_some() {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
        }
        // `Bet > N Pillz:` over a plain Victory body. A printed ability level must be a
        // structural alias of the definition its text resolves to; the one bonus that
        // prints the gate is bridged by clan and catalog id, as Jungo's Victory Life is.
        if classify_bet_gated_post_round(definition, source_kind).is_some() {
            if source_kind == CombatStatEffectSourceV1::Ability {
                require_catalog_alias(
                    match_.alias_ids(),
                    player,
                    hand_slot,
                    source_kind,
                    catalog_id,
                    description,
                    definition,
                )?;
            } else if effective_clan_id != ZENITH_CLAN_ID
                || catalog_id != Some(ZENITH_CATALOG_BONUS_ID)
            {
                return Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
                    player,
                    hand_slot,
                    source_kind,
                    catalog_id,
                    description: description.to_owned(),
                    registry_definition_id: definition.id(),
                    registry_reasons: definition
                        .compiled()
                        .unsupported_reasons()
                        .to_vec()
                        .into_boxed_slice(),
                });
            }
            return prepare_post_round_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
                |definition, source_kind| {
                    let (effect, predicate) =
                        classify_bet_gated_post_round(definition, source_kind)?;
                    let (effect, compact_effect) = effect.effects();
                    Some((effect, compact_effect, predicate))
                },
            );
        }
        // Revision 70's owner-clan gate over the end-of-round bodies. A printed level must be
        // a structural alias of the definition its text resolves to, so the same-text levels
        // that own no registry definition (Dark Kaizerin `4039`/`3350`, Dark Eloxia
        // `5612`/`5614`, Dark Mandrak `5615`) stay closed. The gate itself is judged at
        // resolution from the effective clan this card was prepared with.
        if classify_clan_gated_post_round(definition, source_kind).is_some() {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_post_round_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
                classify_clan_gated_post_round,
            );
        }
        // Revision 73's `Versus` and `After` gates over the plain Victory bodies, by the same
        // rule: a printed level must be a structural alias of the definition its text
        // resolves to, so the same-text levels without a registry definition (Sight Ld
        // `5430`/`3683`, D-aleq `5280`-`5282`, Frau Vanda `5601`, Noma `5668`/`5669`) stay
        // closed. `Versus` is judged from the opposing hand's canonical clans and `After` from
        // the owner's previous card.
        if classify_hand_clan_gated_post_round(definition, source_kind).is_some() {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_post_round_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
                classify_hand_clan_gated_post_round,
            );
        }
        if classify_victory_life(definition, source_kind).is_some() {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_victory_life_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
            );
        }
        // The unconditional opponent-Life reduction follows the same rule on both of its
        // outcome channels. Its identity-locked members - the Berzerk Bonus, the two
        // Confidence records, Doela Noel's Symmetry and Uuber's `1628` - never reach here:
        // their printed texts are intercepted above.
        // Since revision 69 its `Night:` form is admitted too, and a night variant has no
        // catalog id: its exact text, selected in a night match, is its identity.
        if let Some((_, _, predicate)) = classify_victory_opponent_life(definition, source_kind) {
            require_catalog_alias_or_night_variant(
                match_.alias_ids(),
                night,
                predicate,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_victory_opponent_life_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
            );
        }
        if classify_victory_or_defeat_life(definition, source_kind).is_some() {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_victory_or_defeat_life_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
            );
        }
        // Plain Victory Pillz follows the same rule: exact text, complete shape, and a
        // catalog id that is a structural alias of the definition the text resolves to.
        if classify_victory_pillz(definition, source_kind).is_some() {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_victory_pillz_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
            );
        }
        // Its capped form follows the same rule, and its `Night:` form - Nox Ld's night
        // ability - reaches it through the night-variant bridge.
        if let Some((_, _, predicate)) = classify_victory_pillz_max(definition, source_kind) {
            require_catalog_alias_or_night_variant(
                match_.alias_ids(),
                night,
                predicate,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_post_round_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
                |definition, source_kind| {
                    let (pillz, maximum, predicate) =
                        classify_victory_pillz_max(definition, source_kind)?;
                    Some((
                        CombatStatPostRoundEffectV1::GainPillzOnVictoryMax { pillz, maximum },
                        CombatStatEffectV1::GainPillzOnVictoryMax { pillz, maximum },
                        predicate,
                    ))
                },
            );
        }
        // The opposing compound and the larger Victory Or Defeat own gains follow the same
        // rule.
        if classify_victory_opponent_pillz_and_life(definition, source_kind).is_some()
            || classify_victory_or_defeat_pillz_amount(definition, source_kind).is_some()
            || classify_victory_or_defeat_life_per_damage(definition, source_kind).is_some()
        {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_post_round_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
                |definition, source_kind| {
                    if let Some((amount, minimum)) =
                        classify_victory_opponent_pillz_and_life(definition, source_kind)
                    {
                        return Some((
                            CombatStatPostRoundEffectV1::ReduceOpponentPillzAndLifeOnVictory {
                                amount,
                                minimum,
                            },
                            CombatStatEffectV1::ReduceOpponentPillzAndLifeOnVictory {
                                amount,
                                minimum,
                            },
                            CombatStatPredicateV1::Always,
                        ));
                    }
                    if let Some(pillz) =
                        classify_victory_or_defeat_pillz_amount(definition, source_kind)
                    {
                        return Some((
                            CombatStatPostRoundEffectV1::GainPillzOnVictoryOrDefeat { pillz },
                            CombatStatEffectV1::GainPillzOnVictoryOrDefeat { pillz },
                            CombatStatPredicateV1::Always,
                        ));
                    }
                    let life_per_damage =
                        classify_victory_or_defeat_life_per_damage(definition, source_kind)?;
                    Some((
                        CombatStatPostRoundEffectV1::GainLifePerFinalDamageOnVictoryOrDefeat {
                            life_per_damage,
                        },
                        CombatStatEffectV1::GainLifePerFinalDamageOnVictoryOrDefeat {
                            life_per_damage,
                        },
                        CombatStatPredicateV1::Always,
                    ))
                },
            );
        }
        // The opposing Pillz reduction follows the same rule.
        if classify_victory_opponent_pillz(definition, source_kind).is_some() {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_victory_opponent_pillz_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
            );
        }
        // Revision 75's `Stop:` form of it follows the same rule. Its predicate never holds in
        // a match construction admits, which refuses it wherever an opposing source could stop
        // the owner's ability.
        if classify_stop_triggered_opponent_pillz(definition, source_kind).is_some() {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_post_round_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
                |definition, source_kind| {
                    let (pillz, minimum) =
                        classify_stop_triggered_opponent_pillz(definition, source_kind)?;
                    Some((
                        CombatStatPostRoundEffectV1::ReduceOpponentPillzOnVictory {
                            pillz,
                            minimum,
                        },
                        CombatStatEffectV1::ReduceOpponentPillzOnVictory { pillz, minimum },
                        CombatStatPredicateV1::OwnerAbilityStopped,
                    ))
                },
            );
        }
        // And so does its losing-side sibling.
        if classify_defeat_opponent_pillz(definition, source_kind).is_some() {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_defeat_opponent_pillz_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
            );
        }
        // Backlash turns the Min-clamped Life reduction on its owner, and the capped Defeat Life
        // and the Defeat opposing Pillz gift are the Defeat channel's other two corners. All
        // three follow the same alias rule, which keeps Sylvia Ld level 1 closed: it prints
        // level 2's text under catalog id `5491`, which no registry definition owns. Levels
        // whose text no definition carries - Strigoi level 3, Tiwi Ld level 2 - never resolve
        // to reach this arm.
        if classify_backlash_life(definition, source_kind).is_some()
            || classify_defeat_capped_life(definition, source_kind).is_some()
            || classify_defeat_opponent_pillz_gain(definition, source_kind).is_some()
        {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_post_round_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
                |definition, source_kind| {
                    if let Some((life, minimum)) = classify_backlash_life(definition, source_kind) {
                        return Some((
                            CombatStatPostRoundEffectV1::ReduceOwnLifeOnVictory { life, minimum },
                            CombatStatEffectV1::ReduceOwnLifeOnVictory { life, minimum },
                            CombatStatPredicateV1::Always,
                        ));
                    }
                    if let Some((life, maximum)) =
                        classify_defeat_capped_life(definition, source_kind)
                    {
                        return Some((
                            CombatStatPostRoundEffectV1::GainLifeOnDefeatMax { life, maximum },
                            CombatStatEffectV1::GainLifeOnDefeatMax { life, maximum },
                            CombatStatPredicateV1::Always,
                        ));
                    }
                    let pillz = classify_defeat_opponent_pillz_gain(definition, source_kind)?;
                    Some((
                        CombatStatPostRoundEffectV1::GainOpponentPillzOnDefeat { pillz },
                        CombatStatEffectV1::GainOpponentPillzOnDefeat { pillz },
                        CombatStatPredicateV1::Always,
                    ))
                },
            );
        }
        // Corrupt is Xantiax's own half on its own and follows the same alias rule. Nega D Ld
        // is its only printed card.
        if classify_corrupt_own_life(definition, source_kind).is_some() {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_post_round_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
                |definition, source_kind| {
                    let (life, minimum) = classify_corrupt_own_life(definition, source_kind)?;
                    Some((
                        CombatStatPostRoundEffectV1::ReduceOwnLife { life, minimum },
                        CombatStatEffectV1::ReduceOwnLife { life, minimum },
                        CombatStatPredicateV1::Always,
                    ))
                },
            );
        }
        // The post-round Brawl grammars are the plain Victory reductions and gain with an
        // anti-support magnitude, and follow the same rule.
        if classify_brawl_post_round(definition, source_kind).is_some() {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_brawl_post_round_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
            );
        }
        // So are the post-round Support grammars, with the owner's own Support count, and the
        // Equalizer own gains, with the opposing card's stars.
        if classify_support_post_round(definition, source_kind).is_some() {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_post_round_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
                |definition, source_kind| {
                    let (effect, compact_effect) =
                        classify_support_post_round(definition, source_kind)?.effects();
                    Some((effect, compact_effect, CombatStatPredicateV1::Always))
                },
            );
        }
        if classify_equalizer_post_round_gain(definition, source_kind).is_some() {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_post_round_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
                |definition, source_kind| {
                    let (effect, compact_effect) =
                        classify_equalizer_post_round_gain(definition, source_kind)?.effects();
                    Some((effect, compact_effect, CombatStatPredicateV1::Always))
                },
            );
        }
        // The Equalizer opponent-Life grammar beyond its two reviewed card sources, whose
        // `Min 2` text is intercepted by card key above and never reaches this point.
        if classify_equalizer_opponent_life_on_victory(definition, source_kind).is_some() {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_equalizer_opponent_life_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
            );
        }
        // The round-scaled Victory grammars follow the same rule.
        if classify_round_scaled_post_round(definition, source_kind).is_some() {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_post_round_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
                |definition, source_kind| {
                    let (scale, effect) =
                        classify_round_scaled_post_round(definition, source_kind)?;
                    let (effect, compact_effect) = effect.effects(scale);
                    Some((effect, compact_effect, CombatStatPredicateV1::Always))
                },
            );
        }
        // The Killshot compound gain follows the same rule.
        if classify_killshot_pillz_and_life(definition, source_kind).is_some() {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_post_round_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
                |definition, source_kind| {
                    let amount = classify_killshot_pillz_and_life(definition, source_kind)?;
                    Some((
                        CombatStatPostRoundEffectV1::GainPillzAndLifeOnKillshot { amount },
                        CombatStatEffectV1::GainPillzAndLifeOnKillshot { amount },
                        CombatStatPredicateV1::Always,
                    ))
                },
            );
        }
        // And so do its halves on their own, the capped and Unison Life forms and the
        // ratio-latched Toxin. The printed levels without a registry definition of their own
        // (Radamir L1-L3, Superpaquito, Tara L5's `Max. 17`, Madrat L3 and the rest) are not
        // aliases of anything and stay unsupported.
        if classify_killshot_post_round(definition, source_kind).is_some() {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_post_round_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
                |definition, source_kind| {
                    let (effect, predicate) =
                        classify_killshot_post_round(definition, source_kind)?;
                    let (effect, compact_effect) = effect.effects();
                    Some((effect, compact_effect, predicate))
                },
            );
        }
        // So does the Pillz-per-Damage conversion, whose predicate the printed prefix names.
        if classify_victory_pillz_per_damage(definition, source_kind).is_some() {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_victory_pillz_per_damage_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
            );
        }
        if classify_victory_life_per_opponent_damage(definition, source_kind).is_some() {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_post_round_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
                |definition, source_kind| {
                    let life_per_damage =
                        classify_victory_life_per_opponent_damage(definition, source_kind)?;
                    Some((
                        CombatStatPostRoundEffectV1::GainLifePerOpponentFinalDamageOnVictory {
                            life_per_damage,
                        },
                        CombatStatEffectV1::GainLifePerOpponentFinalDamageOnVictory {
                            life_per_damage,
                        },
                        CombatStatPredicateV1::Always,
                    ))
                },
            );
        }
        // And the Life conversion, whose predicate the printed prefix names.
        if classify_victory_life_per_damage(definition, source_kind).is_some() {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_victory_life_per_damage_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
            );
        }
        // Ordinary Defeat Life is generic within its reviewed grammar, but the catalog
        // source must be a real structural alias of the selected registry definition.
        // A same-text row with another numeric identity cannot borrow execution authority.
        if classify_defeat_life(definition, source_kind).is_some()
            || classify_unison_defeat_life(definition, source_kind).is_some()
        {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_defeat_life_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
            );
        }
        // The Unison Victory compound, by the same rule.
        if classify_unison_pillz_and_life(definition, source_kind).is_some() {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_post_round_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
                |definition, source_kind| {
                    let amount = classify_unison_pillz_and_life(definition, source_kind)?;
                    Some((
                        CombatStatPostRoundEffectV1::GainPillzAndLifeOnVictory { amount },
                        CombatStatEffectV1::GainPillzAndLifeOnVictory { amount },
                        CombatStatPredicateV1::OwnerHandUnison,
                    ))
                },
            );
        }
        // Recover is admitted by grammar under the same rule: every printed prefix and
        // ratio, the catalog row a structural alias of the definition its text names. The
        // alias itself stays the executed identity, as revision 8's audited ids always were.
        if classify_recover_pillz(definition, source_kind).is_some() {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_defeat_recover_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                catalog_id.unwrap_or(definition.id()),
            );
        }
        // The Defeat own Pillz gain and its compound follow the same rule.
        if classify_defeat_pillz(definition, source_kind).is_some()
            || classify_defeat_pillz_and_life(definition, source_kind).is_some()
        {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_post_round_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
                |definition, source_kind| {
                    if let Some(pillz) = classify_defeat_pillz(definition, source_kind) {
                        return Some((
                            CombatStatPostRoundEffectV1::GainPillzOnDefeat { pillz },
                            CombatStatEffectV1::GainPillzOnDefeat { pillz },
                            CombatStatPredicateV1::Always,
                        ));
                    }
                    let amount = classify_defeat_pillz_and_life(definition, source_kind)?;
                    Some((
                        CombatStatPostRoundEffectV1::GainPillzAndLifeOnDefeat { amount },
                        CombatStatEffectV1::GainPillzAndLifeOnDefeat { amount },
                        CombatStatPredicateV1::Always,
                    ))
                },
            );
        }
        // The losing-side opponent-Life reduction is generic within its own grammar under
        // the same rule: the catalog row must be a structural alias of the registry
        // definition it claims, so a same-text row with another identity cannot execute.
        if classify_defeat_opponent_life(definition, source_kind).is_some() {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_defeat_opponent_life_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
            );
        }
        // The Killshot reduction is the same grammar on the `sureshot` channel and is
        // admitted by the same alias rule.
        if classify_killshot_opponent_life(definition, source_kind).is_some() {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_killshot_opponent_life_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
            );
        }
        // Xantiax charges both players and names no outcome, but it is admitted by the
        // same rule as every other generic post-round grammar: the catalog row has to be a
        // structural alias of the registry definition its text resolves to.
        if classify_both_players_life_reduction(definition, source_kind).is_some() {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_both_players_life_reduction_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
            );
        }
        // The both-players Victory Or Defeat gains follow the same rule.
        if classify_victory_or_defeat_both_players_gain(definition, source_kind).is_some() {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_post_round_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
                both_players_gain_effect,
            );
        }
        // The plain permanent grammars follow the Victory Life rule: the catalog row must
        // be a structural alias of the registry definition its text resolves to. A
        // same-text row under another numeric identity cannot latch a permanent.
        if classify_heal_life_on_victory(definition, source_kind).is_some()
            || classify_regen_life_on_victory(definition, source_kind).is_some()
            || classify_poison_opponent_life_on_victory(definition, source_kind).is_some()
            || classify_poison_opponent_life_on_defeat(definition, source_kind).is_some()
            || classify_toxin_opponent_life_on_victory(definition, source_kind).is_some()
            || classify_consume_opponent_pillz_on_victory(definition, source_kind).is_some()
            || classify_combust_opponent_life_and_pillz_on_victory(definition, source_kind)
                .is_some()
            || classify_dope_pillz(definition, source_kind).is_some()
            || classify_growth_permanent(definition, source_kind).is_some()
        {
            require_catalog_alias(
                match_.alias_ids(),
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition,
            )?;
            return prepare_permanent_life_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                definition.id(),
            );
        }
        // The compiler recognises the generic neutral Reanimate shape so malformed
        // records remain visible hazards, but catalog execution stays at the one observed
        // Lobo source until another identity has server evidence.
        if classify_reanimate_life(definition, source_kind).is_some() {
            if catalog_id != Some(LOBO_REANIMATE_REGISTRY_ID)
                || !match_.alias_ids().contains(&LOBO_REANIMATE_REGISTRY_ID)
            {
                return Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
                    player,
                    hand_slot,
                    source_kind,
                    catalog_id,
                    description: description.to_owned(),
                    registry_definition_id: definition.id(),
                    registry_reasons: definition
                        .compiled()
                        .unsupported_reasons()
                        .to_vec()
                        .into_boxed_slice(),
                });
            }
            return prepare_reanimate_life_source(
                registry,
                player,
                hand_slot,
                source_kind,
                catalog_id,
                description,
                LOBO_REANIMATE_REGISTRY_ID,
            );
        }
    }
    let match_ = registry.lookup_description(description).map_err(|source| {
        CatalogCombatStatMatchErrorV1::Lookup {
            player,
            hand_slot,
            source_kind,
            catalog_id,
            description: description.to_owned(),
            source,
        }
    })?;
    let definition = match_.definition();
    let Some((effect, predicate)) = classify_combat_stat_effect(definition, source_kind) else {
        return Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            player,
            hand_slot,
            source_kind,
            catalog_id,
            description: description.to_owned(),
            registry_definition_id: definition.id(),
            registry_reasons: definition
                .compiled()
                .unsupported_reasons()
                .to_vec()
                .into_boxed_slice(),
        });
    };
    let compact_effect =
        compact_effect(effect).ok_or(CatalogCombatStatMatchErrorV1::UnsupportedCompiledShape {
            player,
            hand_slot,
            source_kind,
            catalog_id,
            registry_definition_id: definition.id(),
        })?;
    Ok(PreparedCatalogSourceV1 {
        metadata: CatalogCombatStatSourceDispositionV1::Execute {
            identity: CatalogCombatStatModifierIdentityV1 {
                catalog_id,
                description: description.to_owned(),
                registry_definition_id: definition.id(),
                registry_alias_ids: match_.alias_ids().to_vec().into_boxed_slice(),
            },
            effect,
            predicate,
        },
        compact: CombatStatSourcePlanV1::Execute {
            source_id: definition.id(),
            predicate,
            effect: compact_effect,
        },
    })
}

fn prepare_control_source(
    registry: &EffectRegistryV1,
    player: PlayerId,
    hand_slot: HandSlot,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
    description: &str,
    registry_definition_id: u32,
    expected_effect: SupportedEffectV1,
) -> Result<PreparedCatalogSourceV1, CatalogCombatStatMatchErrorV1> {
    let definition = registry
        .lookup_capture(registry_definition_id, description)
        .map_err(|source| CatalogCombatStatMatchErrorV1::Lookup {
            player,
            hand_slot,
            source_kind,
            catalog_id,
            description: description.to_owned(),
            source,
        })?;
    let Some((effect, predicate)) = classify_combat_stat_effect(definition, source_kind) else {
        return Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            player,
            hand_slot,
            source_kind,
            catalog_id,
            description: description.to_owned(),
            registry_definition_id: definition.id(),
            registry_reasons: definition
                .compiled()
                .unsupported_reasons()
                .to_vec()
                .into_boxed_slice(),
        });
    };
    if effect != expected_effect {
        return Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            player,
            hand_slot,
            source_kind,
            catalog_id,
            description: description.to_owned(),
            registry_definition_id: definition.id(),
            registry_reasons: definition
                .compiled()
                .unsupported_reasons()
                .to_vec()
                .into_boxed_slice(),
        });
    }
    let compact_effect =
        compact_effect(effect).ok_or(CatalogCombatStatMatchErrorV1::UnsupportedCompiledShape {
            player,
            hand_slot,
            source_kind,
            catalog_id,
            registry_definition_id: definition.id(),
        })?;
    // Same-text control records are provenance only after their complete structured shape
    // compiles to this exact effect; malformed aliases never inherit execution authority.
    let registry_alias_ids = if description == REPRISAL_STOP_OPPONENT_ABILITY_DESCRIPTION {
        // Reprisal controls are intentionally model-neutrally Unsupported because their
        // defender position is not a generic registry primitive.  Use this projection's
        // exact classifier for provenance only; the catalog-id alias gate above remains
        // the authority for admission.
        registry
            .iter()
            .filter_map(|(id, candidate)| {
                matches!(
                    classify_combat_stat_effect(candidate, source_kind),
                    Some((effect, CombatStatPredicateV1::OwnerMovesSecond))
                        if effect == expected_effect
                )
                .then_some(id)
            })
            .collect::<Vec<_>>()
            .into_boxed_slice()
    } else {
        // Revision 75's Angelo L2 (`877`) is admitted by identity although the registry
        // refuses its record, so it joins the provenance set only as itself.
        registry
            .iter()
            .filter_map(|(id, candidate)| {
                (candidate.description() == description
                    && (candidate.compiled().supported() == Some(expected_effect)
                        || id == definition.id()))
                .then_some(id)
            })
            .collect::<Vec<_>>()
            .into_boxed_slice()
    };
    Ok(PreparedCatalogSourceV1 {
        metadata: CatalogCombatStatSourceDispositionV1::Execute {
            identity: CatalogCombatStatModifierIdentityV1 {
                catalog_id,
                description: description.to_owned(),
                registry_definition_id: definition.id(),
                registry_alias_ids,
            },
            effect,
            predicate,
        },
        compact: CombatStatSourcePlanV1::Execute {
            source_id: definition.id(),
            predicate,
            effect: compact_effect,
        },
    })
}

/// The identity and error plumbing every post-round grammar shares: resolve the registry
/// definition the catalog row claims, ask the grammar to classify it, collect the alias ids
/// of every same-text record, and pair the provenance disposition with the compact plan the
/// hot path executes. A grammar supplies only `classify`, which turns a definition into its
/// two effect representations and its predicate, or `None` to reject it as unsupported.
///
/// Holding this in one place is what keeps the two representations from disagreeing: a
/// grammar returns its public effect and its compact effect together, and the identity they
/// are recorded under is built the same way for every one of them.
/// A generic grammar admits any registry definition carrying its text and shape, so the
/// catalog row has to be a structural alias of the definition its text resolves to: a
/// same-text row under another numeric identity cannot borrow the grammar. Every generic
/// post-round grammar applies this same rule, so they all call it rather than restating it.
fn require_catalog_alias(
    alias_ids: &[u32],
    player: PlayerId,
    hand_slot: HandSlot,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
    description: &str,
    definition: &EffectDefinitionV1,
) -> Result<(), CatalogCombatStatMatchErrorV1> {
    if catalog_id.is_some_and(|id| alias_ids.contains(&id)) {
        return Ok(());
    }
    Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
        player,
        hand_slot,
        source_kind,
        catalog_id,
        description: description.to_owned(),
        registry_definition_id: definition.id(),
        registry_reasons: definition
            .compiled()
            .unsupported_reasons()
            .to_vec()
            .into_boxed_slice(),
    })
}

/// The catalog gives a selected night variant no numeric identity (`derive_catalog_hand`),
/// so a post-round grammar that prints a `Night:` form cannot meet `require_catalog_alias`.
/// Such a source is admitted by its exact text alone, and only when all of these hold: it
/// has no catalog id, the match is at night, its printed text starts with `Night: `, and the
/// grammar resolved it to the `MatchIsNight` predicate. A missing catalog id alone is never
/// enough - a daylight card whose ability id is 0 has none either - and every other source
/// still has to be a structural alias of the definition its text resolves to. This is the
/// rule the conditional Stops and the Night numerics already follow.
fn require_catalog_alias_or_night_variant(
    alias_ids: &[u32],
    night: bool,
    predicate: CombatStatPredicateV1,
    player: PlayerId,
    hand_slot: HandSlot,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
    description: &str,
    definition: &EffectDefinitionV1,
) -> Result<(), CatalogCombatStatMatchErrorV1> {
    if catalog_id.is_none()
        && night
        && description.starts_with("Night: ")
        && predicate == CombatStatPredicateV1::MatchIsNight
    {
        return Ok(());
    }
    require_catalog_alias(
        alias_ids,
        player,
        hand_slot,
        source_kind,
        catalog_id,
        description,
        definition,
    )
}

fn prepare_post_round_source(
    registry: &EffectRegistryV1,
    player: PlayerId,
    hand_slot: HandSlot,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
    description: &str,
    registry_definition_id: u32,
    classify: impl FnOnce(
        &EffectDefinitionV1,
        CombatStatEffectSourceV1,
    ) -> Option<(
        CombatStatPostRoundEffectV1,
        CombatStatEffectV1,
        CombatStatPredicateV1,
    )>,
) -> Result<PreparedCatalogSourceV1, CatalogCombatStatMatchErrorV1> {
    let lookup_failed = |source| CatalogCombatStatMatchErrorV1::Lookup {
        player,
        hand_slot,
        source_kind,
        catalog_id,
        description: description.to_owned(),
        source,
    };
    let definition = registry
        .lookup_capture(registry_definition_id, description)
        .map_err(lookup_failed)?;
    let Some((effect, compact_effect, predicate)) = classify(definition, source_kind) else {
        return Err(CatalogCombatStatMatchErrorV1::UnsupportedSource {
            player,
            hand_slot,
            source_kind,
            catalog_id,
            description: description.to_owned(),
            registry_definition_id: definition.id(),
            registry_reasons: definition
                .compiled()
                .unsupported_reasons()
                .to_vec()
                .into_boxed_slice(),
        });
    };
    let registry_alias_ids = registry
        .lookup_description(description)
        .map_err(lookup_failed)?
        .alias_ids()
        .to_vec()
        .into_boxed_slice();
    Ok(PreparedCatalogSourceV1 {
        metadata: CatalogCombatStatSourceDispositionV1::ExecutePostRound {
            identity: CatalogCombatStatModifierIdentityV1 {
                catalog_id,
                description: description.to_owned(),
                registry_definition_id: definition.id(),
                registry_alias_ids,
            },
            effect,
            predicate,
        },
        compact: CombatStatSourcePlanV1::Execute {
            source_id: definition.id(),
            predicate,
            effect: compact_effect,
        },
    })
}

fn prepare_victory_life_source(
    registry: &EffectRegistryV1,
    player: PlayerId,
    hand_slot: HandSlot,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
    description: &str,
    registry_definition_id: u32,
) -> Result<PreparedCatalogSourceV1, CatalogCombatStatMatchErrorV1> {
    prepare_post_round_source(
        registry,
        player,
        hand_slot,
        source_kind,
        catalog_id,
        description,
        registry_definition_id,
        |definition, source_kind| {
            let (life, predicate) = classify_victory_life(definition, source_kind)?;
            Some((
                CombatStatPostRoundEffectV1::GainLifeOnVictory { life },
                CombatStatEffectV1::GainLifeOnVictory { life },
                predicate,
            ))
        },
    )
}

fn prepare_victory_pillz_source(
    registry: &EffectRegistryV1,
    player: PlayerId,
    hand_slot: HandSlot,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
    description: &str,
    registry_definition_id: u32,
) -> Result<PreparedCatalogSourceV1, CatalogCombatStatMatchErrorV1> {
    prepare_post_round_source(
        registry,
        player,
        hand_slot,
        source_kind,
        catalog_id,
        description,
        registry_definition_id,
        |definition, source_kind| {
            let (pillz, predicate) = classify_victory_pillz(definition, source_kind)?;
            Some((
                CombatStatPostRoundEffectV1::GainPillzOnVictory { pillz },
                CombatStatEffectV1::GainPillzOnVictory { pillz },
                predicate,
            ))
        },
    )
}

fn prepare_victory_opponent_pillz_source(
    registry: &EffectRegistryV1,
    player: PlayerId,
    hand_slot: HandSlot,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
    description: &str,
    registry_definition_id: u32,
) -> Result<PreparedCatalogSourceV1, CatalogCombatStatMatchErrorV1> {
    prepare_post_round_source(
        registry,
        player,
        hand_slot,
        source_kind,
        catalog_id,
        description,
        registry_definition_id,
        |definition, source_kind| {
            let (pillz, minimum) = classify_victory_opponent_pillz(definition, source_kind)?;
            Some((
                CombatStatPostRoundEffectV1::ReduceOpponentPillzOnVictory { pillz, minimum },
                CombatStatEffectV1::ReduceOpponentPillzOnVictory { pillz, minimum },
                CombatStatPredicateV1::Always,
            ))
        },
    )
}

fn prepare_brawl_post_round_source(
    registry: &EffectRegistryV1,
    player: PlayerId,
    hand_slot: HandSlot,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
    description: &str,
    registry_definition_id: u32,
) -> Result<PreparedCatalogSourceV1, CatalogCombatStatMatchErrorV1> {
    prepare_post_round_source(
        registry,
        player,
        hand_slot,
        source_kind,
        catalog_id,
        description,
        registry_definition_id,
        |definition, source_kind| {
            let (effect, compact_effect) =
                classify_brawl_post_round(definition, source_kind)?.effects();
            Some((effect, compact_effect, CombatStatPredicateV1::Always))
        },
    )
}

fn prepare_defeat_opponent_pillz_source(
    registry: &EffectRegistryV1,
    player: PlayerId,
    hand_slot: HandSlot,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
    description: &str,
    registry_definition_id: u32,
) -> Result<PreparedCatalogSourceV1, CatalogCombatStatMatchErrorV1> {
    prepare_post_round_source(
        registry,
        player,
        hand_slot,
        source_kind,
        catalog_id,
        description,
        registry_definition_id,
        |definition, source_kind| {
            let (pillz, minimum) = classify_defeat_opponent_pillz(definition, source_kind)?;
            Some((
                CombatStatPostRoundEffectV1::ReduceOpponentPillzOnDefeat { pillz, minimum },
                CombatStatEffectV1::ReduceOpponentPillzOnDefeat { pillz, minimum },
                CombatStatPredicateV1::Always,
            ))
        },
    )
}

fn prepare_victory_pillz_per_damage_source(
    registry: &EffectRegistryV1,
    player: PlayerId,
    hand_slot: HandSlot,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
    description: &str,
    registry_definition_id: u32,
) -> Result<PreparedCatalogSourceV1, CatalogCombatStatMatchErrorV1> {
    prepare_post_round_source(
        registry,
        player,
        hand_slot,
        source_kind,
        catalog_id,
        description,
        registry_definition_id,
        |definition, source_kind| {
            let predicate = classify_victory_pillz_per_damage(definition, source_kind)?;
            Some((
                CombatStatPostRoundEffectV1::GainPillzEqualToFinalDamageOnVictory,
                CombatStatEffectV1::GainPillzEqualToFinalDamageOnVictory,
                predicate,
            ))
        },
    )
}

fn prepare_victory_life_per_damage_source(
    registry: &EffectRegistryV1,
    player: PlayerId,
    hand_slot: HandSlot,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
    description: &str,
    registry_definition_id: u32,
) -> Result<PreparedCatalogSourceV1, CatalogCombatStatMatchErrorV1> {
    prepare_post_round_source(
        registry,
        player,
        hand_slot,
        source_kind,
        catalog_id,
        description,
        registry_definition_id,
        |definition, source_kind| {
            let (life_per_damage, maximum, predicate) =
                classify_victory_life_per_damage(definition, source_kind)?;
            Some((
                CombatStatPostRoundEffectV1::GainLifePerFinalDamageOnVictory {
                    life_per_damage,
                    maximum,
                },
                CombatStatEffectV1::GainLifePerFinalDamageOnVictory {
                    life_per_damage,
                    maximum,
                },
                predicate,
            ))
        },
    )
}

fn prepare_defeat_life_source(
    registry: &EffectRegistryV1,
    player: PlayerId,
    hand_slot: HandSlot,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
    description: &str,
    registry_definition_id: u32,
) -> Result<PreparedCatalogSourceV1, CatalogCombatStatMatchErrorV1> {
    prepare_post_round_source(
        registry,
        player,
        hand_slot,
        source_kind,
        catalog_id,
        description,
        registry_definition_id,
        |definition, source_kind| {
            let (life, predicate) = classify_defeat_life(definition, source_kind)
                .map(|life| (life, CombatStatPredicateV1::Always))
                .or_else(|| {
                    classify_unison_defeat_life(definition, source_kind)
                        .map(|life| (life, CombatStatPredicateV1::OwnerHandUnison))
                })?;
            Some((
                CombatStatPostRoundEffectV1::GainLifeOnDefeat { life },
                CombatStatEffectV1::GainLifeOnDefeat { life },
                predicate,
            ))
        },
    )
}

fn prepare_defeat_opponent_life_source(
    registry: &EffectRegistryV1,
    player: PlayerId,
    hand_slot: HandSlot,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
    description: &str,
    registry_definition_id: u32,
) -> Result<PreparedCatalogSourceV1, CatalogCombatStatMatchErrorV1> {
    prepare_post_round_source(
        registry,
        player,
        hand_slot,
        source_kind,
        catalog_id,
        description,
        registry_definition_id,
        |definition, source_kind| {
            let (life, minimum) = classify_defeat_opponent_life(definition, source_kind)?;
            Some((
                CombatStatPostRoundEffectV1::ReduceOpponentLifeOnDefeat { life, minimum },
                CombatStatEffectV1::ReduceOpponentLifeOnDefeat { life, minimum },
                CombatStatPredicateV1::Always,
            ))
        },
    )
}

fn prepare_killshot_opponent_life_source(
    registry: &EffectRegistryV1,
    player: PlayerId,
    hand_slot: HandSlot,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
    description: &str,
    registry_definition_id: u32,
) -> Result<PreparedCatalogSourceV1, CatalogCombatStatMatchErrorV1> {
    prepare_post_round_source(
        registry,
        player,
        hand_slot,
        source_kind,
        catalog_id,
        description,
        registry_definition_id,
        |definition, source_kind| {
            let (life, minimum) = classify_killshot_opponent_life(definition, source_kind)?;
            Some((
                CombatStatPostRoundEffectV1::ReduceOpponentLifeOnKillshot { life, minimum },
                CombatStatEffectV1::ReduceOpponentLifeOnKillshot { life, minimum },
                CombatStatPredicateV1::Always,
            ))
        },
    )
}

fn prepare_both_players_life_reduction_source(
    registry: &EffectRegistryV1,
    player: PlayerId,
    hand_slot: HandSlot,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
    description: &str,
    registry_definition_id: u32,
) -> Result<PreparedCatalogSourceV1, CatalogCombatStatMatchErrorV1> {
    prepare_post_round_source(
        registry,
        player,
        hand_slot,
        source_kind,
        catalog_id,
        description,
        registry_definition_id,
        |definition, source_kind| {
            let (life, minimum) = classify_both_players_life_reduction(definition, source_kind)?;
            Some((
                CombatStatPostRoundEffectV1::ReduceBothPlayersLife { life, minimum },
                CombatStatEffectV1::ReduceBothPlayersLife { life, minimum },
                CombatStatPredicateV1::Always,
            ))
        },
    )
}

fn both_players_gain_effect(
    definition: &EffectDefinitionV1,
    source_kind: CombatStatEffectSourceV1,
) -> Option<(
    CombatStatPostRoundEffectV1,
    CombatStatEffectV1,
    CombatStatPredicateV1,
)> {
    let (resource, amount) = classify_victory_or_defeat_both_players_gain(definition, source_kind)?;
    Some(match resource {
        BothPlayersGainV1::Life => (
            CombatStatPostRoundEffectV1::GainBothPlayersLifeOnVictoryOrDefeat { life: amount },
            CombatStatEffectV1::GainBothPlayersLifeOnVictoryOrDefeat { life: amount },
            CombatStatPredicateV1::Always,
        ),
        BothPlayersGainV1::Pillz => (
            CombatStatPostRoundEffectV1::GainBothPlayersPillzOnVictoryOrDefeat { pillz: amount },
            CombatStatEffectV1::GainBothPlayersPillzOnVictoryOrDefeat { pillz: amount },
            CombatStatPredicateV1::Always,
        ),
    })
}

fn prepare_reanimate_life_source(
    registry: &EffectRegistryV1,
    player: PlayerId,
    hand_slot: HandSlot,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
    description: &str,
    registry_definition_id: u32,
) -> Result<PreparedCatalogSourceV1, CatalogCombatStatMatchErrorV1> {
    prepare_post_round_source(
        registry,
        player,
        hand_slot,
        source_kind,
        catalog_id,
        description,
        registry_definition_id,
        |definition, source_kind| {
            let life = classify_reanimate_life(definition, source_kind)?;
            Some((
                CombatStatPostRoundEffectV1::ReanimateLife { life },
                CombatStatEffectV1::ReanimateLife { life },
                CombatStatPredicateV1::Always,
            ))
        },
    )
}

/// Resolves only the reviewed recovery identities. In particular, catalog bonus id 43 is
/// the Vortex catalog identifier, not an effect-registry definition id; its bridge is valid
/// only for an active effective Vortex bonus and pins registry definition 577.
fn defeat_recover_registry_definition_id(
    source_kind: CombatStatEffectSourceV1,
    effective_clan_id: u32,
    catalog_id: Option<u32>,
    description: &str,
) -> Option<u32> {
    if description != DEFEAT_RECOVER_DESCRIPTION {
        return None;
    }
    // Card abilities are admitted by grammar below, as a structural alias of the
    // definition their text resolves to; only the clan bonus needs this bridge.
    match (source_kind, catalog_id) {
        (CombatStatEffectSourceV1::Bonus, Some(VORTEX_CATALOG_BONUS_ID))
            if effective_clan_id == VORTEX_CLAN_ID =>
        {
            Some(577)
        }
        _ => None,
    }
}

fn prepare_defeat_recover_source(
    registry: &EffectRegistryV1,
    player: PlayerId,
    hand_slot: HandSlot,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
    description: &str,
    registry_definition_id: u32,
) -> Result<PreparedCatalogSourceV1, CatalogCombatStatMatchErrorV1> {
    prepare_post_round_source(
        registry,
        player,
        hand_slot,
        source_kind,
        catalog_id,
        description,
        registry_definition_id,
        |definition, source_kind| {
            let recover = classify_recover_pillz(definition, source_kind)?;
            let (post_round, effect) = recover.effects();
            Some((post_round, effect, recover.predicate))
        },
    )
}

/// One of the four plain permanent grammars, already resolved to the registry definition
/// that owns it. The typed pair is rebuilt here from the same classifiers so a definition
/// that matches none of them is refused rather than guessed.
fn prepare_permanent_life_source(
    registry: &EffectRegistryV1,
    player: PlayerId,
    hand_slot: HandSlot,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
    description: &str,
    registry_definition_id: u32,
) -> Result<PreparedCatalogSourceV1, CatalogCombatStatMatchErrorV1> {
    prepare_post_round_source(
        registry,
        player,
        hand_slot,
        source_kind,
        catalog_id,
        description,
        registry_definition_id,
        // The four plain permanents share one latch and one shape family, so they are tried
        // in turn here rather than given four near-identical preparers of their own.
        |definition, source_kind| {
            // Revision 74: the `Growth:` permanents, whose latched amount the engine freezes
            // in the round that latches them.
            if let Some((effect, compact_effect)) =
                classify_growth_permanent(definition, source_kind)
            {
                return Some((effect, compact_effect, CombatStatPredicateV1::Always));
            }
            if let Some((pillz, maximum, latch)) = classify_dope_pillz(definition, source_kind) {
                let (effect, compact_effect) = latch.effects(pillz, maximum);
                return Some((effect, compact_effect, CombatStatPredicateV1::Always));
            }
            if let Some((life, maximum, predicate)) =
                classify_heal_life_on_victory(definition, source_kind)
            {
                return Some((
                    CombatStatPostRoundEffectV1::HealLifeOnVictory { life, maximum },
                    CombatStatEffectV1::HealLifeOnVictory { life, maximum },
                    predicate,
                ));
            }
            if let Some((life, maximum, predicate)) =
                classify_regen_life_on_victory(definition, source_kind)
            {
                return Some((
                    CombatStatPostRoundEffectV1::RegenLifeOnVictory { life, maximum },
                    CombatStatEffectV1::RegenLifeOnVictory { life, maximum },
                    predicate,
                ));
            }
            if let Some((life, minimum, predicate)) =
                classify_poison_opponent_life_on_victory(definition, source_kind)
            {
                return Some((
                    CombatStatPostRoundEffectV1::PoisonOpponentLifeOnVictory { life, minimum },
                    CombatStatEffectV1::PoisonOpponentLifeOnVictory { life, minimum },
                    predicate,
                ));
            }
            // The losing-side latch carries no condition of its own: the outcome is the
            // condition, and the engine resolves it.
            if let Some((life, minimum)) =
                classify_poison_opponent_life_on_defeat(definition, source_kind)
            {
                return Some((
                    CombatStatPostRoundEffectV1::PoisonOpponentLifeOnDefeat { life, minimum },
                    CombatStatEffectV1::PoisonOpponentLifeOnDefeat { life, minimum },
                    CombatStatPredicateV1::Always,
                ));
            }
            if let Some((pillz, minimum, predicate)) =
                classify_consume_opponent_pillz_on_victory(definition, source_kind)
            {
                return Some((
                    CombatStatPostRoundEffectV1::ConsumeOpponentPillzOnVictory { pillz, minimum },
                    CombatStatEffectV1::ConsumeOpponentPillzOnVictory { pillz, minimum },
                    predicate,
                ));
            }
            if let Some((amount, minimum, predicate)) =
                classify_combust_opponent_life_and_pillz_on_victory(definition, source_kind)
            {
                return Some((
                    CombatStatPostRoundEffectV1::CombustOpponentLifeAndPillzOnVictory {
                        amount,
                        minimum,
                    },
                    CombatStatEffectV1::CombustOpponentLifeAndPillzOnVictory { amount, minimum },
                    predicate,
                ));
            }
            let (life, minimum, predicate) =
                classify_toxin_opponent_life_on_victory(definition, source_kind)?;
            Some((
                CombatStatPostRoundEffectV1::ToxinOpponentLifeOnVictory { life, minimum },
                CombatStatEffectV1::ToxinOpponentLifeOnVictory { life, minimum },
                predicate,
            ))
        },
    )
}

fn prepare_argos_defeat_capped_pillz_source(
    registry: &EffectRegistryV1,
    player: PlayerId,
    hand_slot: HandSlot,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
    description: &str,
) -> Result<PreparedCatalogSourceV1, CatalogCombatStatMatchErrorV1> {
    prepare_post_round_source(
        registry,
        player,
        hand_slot,
        source_kind,
        catalog_id,
        description,
        ARGOS_DEFEAT_CAPPED_PILLZ_REGISTRY_ID,
        |definition, source_kind| {
            classify_argos_defeat_capped_pillz(definition, source_kind).then_some((
                CombatStatPostRoundEffectV1::GainTwoPillzOnDefeatMaxEleven,
                CombatStatEffectV1::GainTwoPillzOnDefeatMaxEleven,
                CombatStatPredicateV1::Always,
            ))
        },
    )
}

fn victory_or_defeat_life_description(description: &str) -> bool {
    matches!(
        description,
        VICTORY_OR_DEFEAT_GAIN_LIFE_ONE_DESCRIPTION
            | VICTORY_OR_DEFEAT_GAIN_LIFE_TWO_DESCRIPTION
            | VICTORY_OR_DEFEAT_REDUCE_OPPONENT_LIFE_DESCRIPTION
    )
}

/// Maps canonical card provenance to the exact captured definition which the cold
/// compiler audits. Zerkov levels three and four have catalog-local ids 5800/5801 but
/// their shared captured definition is 5799; no other same-text alias inherits it.
fn victory_or_defeat_life_registry_definition_id(
    card_key: CardKey,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
    description: &str,
) -> Option<u32> {
    if source_kind != CombatStatEffectSourceV1::Ability {
        return None;
    }
    match (card_key, catalog_id, description) {
        (
            CardKey {
                id: 1586,
                level: 2..=4,
            },
            Some(1396),
            VICTORY_OR_DEFEAT_GAIN_LIFE_ONE_DESCRIPTION,
        ) => Some(1396),
        (
            CardKey { id: 820, level: 3 },
            Some(5835),
            VICTORY_OR_DEFEAT_GAIN_LIFE_ONE_DESCRIPTION,
        ) => Some(5835),
        (
            CardKey { id: 820, level: 4 },
            Some(2992),
            VICTORY_OR_DEFEAT_GAIN_LIFE_ONE_DESCRIPTION,
        ) => Some(2992),
        (
            CardKey {
                id: 2693,
                level: 2..=4,
            },
            Some(5799 | 5800 | 5801),
            VICTORY_OR_DEFEAT_GAIN_LIFE_ONE_DESCRIPTION,
        ) => Some(5799),
        (
            CardKey { id: 2693, level: 5 },
            Some(5802),
            VICTORY_OR_DEFEAT_GAIN_LIFE_TWO_DESCRIPTION,
        ) => Some(5802),
        (
            CardKey { id: 1676, level: 2 },
            Some(2944),
            VICTORY_OR_DEFEAT_GAIN_LIFE_TWO_DESCRIPTION,
        ) => Some(2944),
        (
            CardKey { id: 1788, level: 2 },
            Some(1628),
            VICTORY_OR_DEFEAT_REDUCE_OPPONENT_LIFE_DESCRIPTION,
        ) => Some(1628),
        _ => None,
    }
}

fn prepare_victory_or_defeat_life_source(
    registry: &EffectRegistryV1,
    player: PlayerId,
    hand_slot: HandSlot,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
    description: &str,
    registry_definition_id: u32,
) -> Result<PreparedCatalogSourceV1, CatalogCombatStatMatchErrorV1> {
    prepare_post_round_source(
        registry,
        player,
        hand_slot,
        source_kind,
        catalog_id,
        description,
        registry_definition_id,
        |definition, source_kind| {
            let (effect, compact_effect) =
                match classify_victory_or_defeat_life(definition, source_kind)? {
                    VictoryOrDefeatLifeEffectV1::GainLife { life } => (
                        CombatStatPostRoundEffectV1::GainLifeOnVictoryOrDefeat { life },
                        CombatStatEffectV1::GainLifeOnVictoryOrDefeat { life },
                    ),
                    VictoryOrDefeatLifeEffectV1::ReduceOpponentLife { life, minimum } => (
                        CombatStatPostRoundEffectV1::ReduceOpponentLifeOnVictoryOrDefeat {
                            life,
                            minimum,
                        },
                        CombatStatEffectV1::ReduceOpponentLifeOnVictoryOrDefeat { life, minimum },
                    ),
                };
            Some((effect, compact_effect, CombatStatPredicateV1::Always))
        },
    )
}

/// Maps the only two printed catalog sources reviewed for Equalizer opponent-Life. The
/// IDs share a registry description alias group, but neither aliases nor Copy provenance
/// authorize another catalog row.
fn equalizer_opponent_life_registry_definition_id(
    card_key: CardKey,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
) -> Option<u32> {
    match (card_key, source_kind, catalog_id) {
        (CardKey { id: 1605, level: 2 }, CombatStatEffectSourceV1::Ability, Some(1415)) => {
            Some(1415)
        }
        (CardKey { id: 841, level: 2 }, CombatStatEffectSourceV1::Ability, Some(4458)) => {
            Some(4458)
        }
        _ => None,
    }
}

fn prepare_equalizer_opponent_life_source(
    registry: &EffectRegistryV1,
    player: PlayerId,
    hand_slot: HandSlot,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
    description: &str,
    registry_definition_id: u32,
) -> Result<PreparedCatalogSourceV1, CatalogCombatStatMatchErrorV1> {
    prepare_post_round_source(
        registry,
        player,
        hand_slot,
        source_kind,
        catalog_id,
        description,
        registry_definition_id,
        |definition, source_kind| {
            let (per_star, minimum) =
                classify_equalizer_opponent_life_on_victory(definition, source_kind)?;
            Some((
                CombatStatPostRoundEffectV1::ReduceOpponentLifeOnVictoryPerOpponentStars {
                    per_star,
                    minimum,
                },
                CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerOpponentStars {
                    per_star,
                    minimum,
                },
                CombatStatPredicateV1::Always,
            ))
        },
    )
}

/// Catalog clan-bonus id 47 is not a capture registry id. It maps to the reviewed Riots
/// definition only after effective-clan activation. Printed ability ids map only to their
/// exact same registry definitions; notably, no catalog card can synthesize dynamic 1034.
/// Catalog authority for the reviewed identities. A printed ability must be the exact card
/// key and printed ability id; Berzerk must be the active effective clan with its catalog
/// bonus id. Neither the registry's text lookup nor a card from another clan can
/// substitute, and a Copy cannot synthesize any of them.
fn victory_opponent_life_registry_definition_id(
    card_key: CardKey,
    source_kind: CombatStatEffectSourceV1,
    effective_clan_id: u32,
    catalog_id: Option<u32>,
    description: &str,
) -> Option<u32> {
    match (source_kind, catalog_id) {
        (CombatStatEffectSourceV1::Ability, Some(id)) => VICTORY_OPPONENT_LIFE_ABILITY_CARDS
            .iter()
            .find(|(key, registry_id, text)| {
                *key == card_key && *registry_id == id && *text == description
            })
            .map(|(_, registry_id, _)| *registry_id),
        (CombatStatEffectSourceV1::Bonus, Some(BERZERK_CATALOG_BONUS_ID))
            if effective_clan_id == BERZERK_CLAN_ID
                && description == BERZERK_VICTORY_OPPONENT_LIFE_DESCRIPTION =>
        {
            Some(BERZERK_VICTORY_OPPONENT_LIFE_REGISTRY_ID)
        }
        _ => None,
    }
}

fn prepare_victory_opponent_life_source(
    registry: &EffectRegistryV1,
    player: PlayerId,
    hand_slot: HandSlot,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
    description: &str,
    registry_definition_id: u32,
) -> Result<PreparedCatalogSourceV1, CatalogCombatStatMatchErrorV1> {
    prepare_post_round_source(
        registry,
        player,
        hand_slot,
        source_kind,
        catalog_id,
        description,
        registry_definition_id,
        |definition, source_kind| {
            let (life, minimum, predicate) =
                classify_victory_opponent_life(definition, source_kind)?;
            Some((
                CombatStatPostRoundEffectV1::ReduceOpponentLifeOnVictory { life, minimum },
                CombatStatEffectV1::ReduceOpponentLifeOnVictory { life, minimum },
                predicate,
            ))
        },
    )
}

fn victory_or_defeat_pillz_registry_definition_id(
    source_kind: CombatStatEffectSourceV1,
    effective_clan_id: u32,
    catalog_id: Option<u32>,
) -> Option<u32> {
    match (source_kind, catalog_id) {
        (CombatStatEffectSourceV1::Bonus, Some(RIOTS_CATALOG_BONUS_ID))
            if effective_clan_id == RIOTS_CLAN_ID =>
        {
            Some(VICTORY_OR_DEFEAT_RIOTS_BONUS_REGISTRY_ID)
        }
        (CombatStatEffectSourceV1::Ability, Some(id @ (1375 | 4111 | 5085 | 5520))) => Some(id),
        _ => None,
    }
}

fn prepare_victory_or_defeat_pillz_source(
    registry: &EffectRegistryV1,
    player: PlayerId,
    hand_slot: HandSlot,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
    description: &str,
    registry_definition_id: u32,
) -> Result<PreparedCatalogSourceV1, CatalogCombatStatMatchErrorV1> {
    prepare_post_round_source(
        registry,
        player,
        hand_slot,
        source_kind,
        catalog_id,
        description,
        registry_definition_id,
        |definition, source_kind| {
            classify_victory_or_defeat_pillz(definition, source_kind).then_some((
                CombatStatPostRoundEffectV1::GainOnePillzOnVictoryOrDefeat,
                CombatStatEffectV1::GainOnePillzOnVictoryOrDefeat,
                CombatStatPredicateV1::Always,
            ))
        },
    )
}

/// The immutable catalog can only execute Anita's printed level-three Ability. The
/// registry's shared text lookup is deliberately not authority: Ellie and Lorea, an
/// invented Bonus, and dynamic Copy must remain outside this narrow catalog slice.
fn anita_courage_damage_to_life_registry_definition_id(
    card_key: CardKey,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
) -> Option<u32> {
    match (card_key, source_kind, catalog_id) {
        (
            CardKey { id: 448, level: 3 },
            CombatStatEffectSourceV1::Ability,
            Some(ANITA_COURAGE_DAMAGE_TO_LIFE_REGISTRY_ID),
        ) => Some(ANITA_COURAGE_DAMAGE_TO_LIFE_REGISTRY_ID),
        _ => None,
    }
}

fn prepare_anita_courage_damage_to_life_source(
    registry: &EffectRegistryV1,
    player: PlayerId,
    hand_slot: HandSlot,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
    description: &str,
) -> Result<PreparedCatalogSourceV1, CatalogCombatStatMatchErrorV1> {
    prepare_post_round_source(
        registry,
        player,
        hand_slot,
        source_kind,
        catalog_id,
        description,
        ANITA_COURAGE_DAMAGE_TO_LIFE_REGISTRY_ID,
        |definition, source_kind| {
            classify_anita_courage_damage_to_life(definition, source_kind).then_some((
                CombatStatPostRoundEffectV1::GainLifeEqualToFinalDamageOnCourageVictory,
                CombatStatEffectV1::GainLifeEqualToFinalDamageOnCourageVictory,
                CombatStatPredicateV1::OwnerMovesFirst,
            ))
        },
    )
}

fn prepare_komboka_victory_pillz_and_life_source(
    registry: &EffectRegistryV1,
    player: PlayerId,
    hand_slot: HandSlot,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
    description: &str,
) -> Result<PreparedCatalogSourceV1, CatalogCombatStatMatchErrorV1> {
    prepare_post_round_source(
        registry,
        player,
        hand_slot,
        source_kind,
        catalog_id,
        description,
        KOMBOKA_VICTORY_PILLZ_AND_LIFE_REGISTRY_ID,
        |definition, source_kind| {
            classify_komboka_victory_pillz_and_life(definition, source_kind).then_some((
                CombatStatPostRoundEffectV1::GainOnePillzAndLifeOnVictory,
                CombatStatEffectV1::GainOnePillzAndLifeOnVictory,
                CombatStatPredicateV1::Always,
            ))
        },
    )
}
