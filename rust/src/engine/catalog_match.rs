//! Catalog-only construction for fully executable combat-stat matches.
//!
//! This is deliberately separate from replay preparation. It derives immutable whole-draw
//! context from canonical cards, resolves printed descriptions through the reviewed effect
//! registry, and refuses a match if any legal card could reach an unsupported effect.

use super::combat_stat_compiler::{
    classify_anita_courage_damage_to_life, classify_argos_defeat_capped_pillz,
    classify_combat_stat_effect, classify_defeat_life, classify_defeat_recover_pillz,
    classify_equalizer_opponent_life_on_victory, classify_komboka_victory_pillz_and_life,
    classify_reanimate_life, classify_victory_life, classify_victory_opponent_life,
    classify_victory_or_defeat_life, classify_victory_or_defeat_pillz, compact_effect,
    VictoryOrDefeatLifeEffectV1, COMBAT_STAT_COMPILER_POLICY_SEMANTIC_REVISION_V1,
};
use super::{
    BaseRulesCardSpec, BaseRulesMatchSpec, BaseRulesPlayerSpec, ByPlayer, CombatStatCardPlanV1,
    CombatStatDiagnosticMatchSpecV1, CombatStatDiagnosticV1, CombatStatEffectSourceV1,
    CombatStatEffectV1, CombatStatMagnitudeV1, CombatStatPlanErrorV1, CombatStatPostRoundEffectV1,
    CombatStatPredicateV1, CombatStatSourcePlanV1, HandSlot, PlayerId, HAND_SIZE,
};
use crate::catalog::{
    CanonicalCard, CardCatalog, CardKey, EffectiveCardCatalog,
    EffectiveCatalogSourceFingerprintFnv1a64,
};
use crate::effect_registry::{
    EffectLookupError, EffectRegistryV1, SourceFingerprintFnv1a64, SupportedEffectV1,
    UnsupportedReasonV1,
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
const MOU_VICTORY_OPPONENT_LIFE_REGISTRY_ID: u32 = 1399;
const MOU_VICTORY_OPPONENT_LIFE_DESCRIPTION: &str = "-5 Opp. Life Min 5";
const MOU_CARD: CardKey = CardKey { id: 1589, level: 3 };
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
const JUNGO_CATALOG_BONUS_ID: u32 = 41;
const JUNGO_VICTORY_LIFE_BONUS_REGISTRY_ID: u32 = 401;
const JUNGO_VICTORY_LIFE_DESCRIPTION: &str = "+2 Life";
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
pub const CATALOG_CONTEXT_POLICY_SEMANTIC_REVISION_V1: u16 = 3;

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

        let match_spec = CombatStatDiagnosticMatchSpecV1 {
            base_rules: BaseRulesMatchSpec {
                battle_rule_id: input.battle_rule_id,
                night: input.night,
                players: base_players.map(|player| player.expect("both players were prepared")),
            },
            cards: compact_cards
                .map(|hand| hand.map(|card| card.expect("all eight compact cards were prepared"))),
        };
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
    matches!(
        plan,
        CombatStatSourcePlanV1::Execute {
            effect: CombatStatEffectV1::ModifyCombatStat {
                multiplier: CombatStatMagnitudeV1::SourceBonusSupport,
                ..
            },
            ..
        }
    )
    .then_some(effective_clan_character_count)
    .unwrap_or(0)
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
    catalog_id: Option<u32>,
    description: &str,
) -> Result<PreparedCatalogSourceV1, CatalogCombatStatMatchErrorV1> {
    if matches!(description, "No Ability" | "No Bonus") {
        return Ok(absent_source());
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
    // Unconditional Victory opponent-Life has exactly two reviewed catalog identities:
    // Mou level three's printed Ability and the active Berzerk clan Bonus. Same-text
    // catalog ids (Rakhan 978, Milovan 498, Fraser 1289) carry no registry definition, and
    // description equality never transfers this effect to another card or clan.
    if description == MOU_VICTORY_OPPONENT_LIFE_DESCRIPTION
        || description == BERZERK_VICTORY_OPPONENT_LIFE_DESCRIPTION
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
    // Equalizer opponent-Life has only two reviewed canonical printed abilities. Dynamic
    // Copy is admitted by replay preparation, never by this immutable catalog constructor.
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
    // Komboka's clan bonus is a catalog namespace source.  Its same-text Carnibox
    // Ability:3356 alias is provenance only: it must never lend execution authority to
    // a card ability, another clan, or a different catalog bonus id.
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
    // Every other generic Victory Life source must carry an actual registry identity in
    // the catalog. Description equality alone is never authority to execute a Life effect.
    if let Ok(match_) = registry.lookup_description(description) {
        let definition = match_.definition();
        if classify_victory_life(definition, source_kind).is_some() {
            if !catalog_id.is_some_and(|id| match_.alias_ids().contains(&id)) {
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
        // Ordinary Defeat Life is generic within its reviewed grammar, but the catalog
        // source must be a real structural alias of the selected registry definition.
        // A same-text row with another numeric identity cannot borrow execution authority.
        if classify_defeat_life(definition, source_kind).is_some() {
            if !catalog_id.is_some_and(|id| match_.alias_ids().contains(&id)) {
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
        registry
            .iter()
            .filter_map(|(id, candidate)| {
                (candidate.description() == description
                    && candidate.compiled().supported() == Some(expected_effect))
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

fn prepare_victory_life_source(
    registry: &EffectRegistryV1,
    player: PlayerId,
    hand_slot: HandSlot,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
    description: &str,
    registry_definition_id: u32,
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
    let Some(life) = classify_victory_life(definition, source_kind) else {
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
    Ok(PreparedCatalogSourceV1 {
        metadata: CatalogCombatStatSourceDispositionV1::ExecutePostRound {
            identity: CatalogCombatStatModifierIdentityV1 {
                catalog_id,
                description: description.to_owned(),
                registry_definition_id: definition.id(),
                registry_alias_ids,
            },
            effect: CombatStatPostRoundEffectV1::GainLifeOnVictory { life },
        },
        compact: CombatStatSourcePlanV1::Execute {
            source_id: definition.id(),
            predicate: CombatStatPredicateV1::Always,
            effect: CombatStatEffectV1::GainLifeOnVictory { life },
        },
    })
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
    let Some(life) = classify_defeat_life(definition, source_kind) else {
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
    Ok(PreparedCatalogSourceV1 {
        metadata: CatalogCombatStatSourceDispositionV1::ExecutePostRound {
            identity: CatalogCombatStatModifierIdentityV1 {
                catalog_id,
                description: description.to_owned(),
                registry_definition_id: definition.id(),
                registry_alias_ids,
            },
            effect: CombatStatPostRoundEffectV1::GainLifeOnDefeat { life },
        },
        compact: CombatStatSourcePlanV1::Execute {
            source_id: definition.id(),
            predicate: CombatStatPredicateV1::Always,
            effect: CombatStatEffectV1::GainLifeOnDefeat { life },
        },
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
    let Some(life) = classify_reanimate_life(definition, source_kind) else {
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
    Ok(PreparedCatalogSourceV1 {
        metadata: CatalogCombatStatSourceDispositionV1::ExecutePostRound {
            identity: CatalogCombatStatModifierIdentityV1 {
                catalog_id,
                description: description.to_owned(),
                registry_definition_id: definition.id(),
                registry_alias_ids,
            },
            effect: CombatStatPostRoundEffectV1::ReanimateLife { life },
        },
        compact: CombatStatSourcePlanV1::Execute {
            source_id: definition.id(),
            predicate: CombatStatPredicateV1::Always,
            effect: CombatStatEffectV1::ReanimateLife { life },
        },
    })
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
    match (source_kind, catalog_id) {
        (CombatStatEffectSourceV1::Ability, Some(id @ (729 | 1418))) => Some(id),
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
    if !classify_defeat_recover_pillz(definition, source_kind) {
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
    // Report the registry's actual structurally identical aliases as provenance. Admission
    // remains pinned to `registry_definition_id`; aliases never inherit executability.
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
    Ok(PreparedCatalogSourceV1 {
        metadata: CatalogCombatStatSourceDispositionV1::ExecutePostRound {
            identity: CatalogCombatStatModifierIdentityV1 {
                catalog_id,
                description: description.to_owned(),
                registry_definition_id: definition.id(),
                registry_alias_ids,
            },
            effect: CombatStatPostRoundEffectV1::RecoverPaidPillzOnDefeat,
        },
        compact: CombatStatSourcePlanV1::Execute {
            source_id: definition.id(),
            predicate: CombatStatPredicateV1::Always,
            effect: CombatStatEffectV1::RecoverPaidPillzOnDefeat,
        },
    })
}

fn prepare_argos_defeat_capped_pillz_source(
    registry: &EffectRegistryV1,
    player: PlayerId,
    hand_slot: HandSlot,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
    description: &str,
) -> Result<PreparedCatalogSourceV1, CatalogCombatStatMatchErrorV1> {
    let definition = registry
        .lookup_capture(ARGOS_DEFEAT_CAPPED_PILLZ_REGISTRY_ID, description)
        .map_err(|source| CatalogCombatStatMatchErrorV1::Lookup {
            player,
            hand_slot,
            source_kind,
            catalog_id,
            description: description.to_owned(),
            source,
        })?;
    if !classify_argos_defeat_capped_pillz(definition, source_kind) {
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
    Ok(PreparedCatalogSourceV1 {
        metadata: CatalogCombatStatSourceDispositionV1::ExecutePostRound {
            identity: CatalogCombatStatModifierIdentityV1 {
                catalog_id,
                description: description.to_owned(),
                registry_definition_id: definition.id(),
                registry_alias_ids,
            },
            effect: CombatStatPostRoundEffectV1::GainTwoPillzOnDefeatMaxEleven,
        },
        compact: CombatStatSourcePlanV1::Execute {
            source_id: definition.id(),
            predicate: CombatStatPredicateV1::Always,
            effect: CombatStatEffectV1::GainTwoPillzOnDefeatMaxEleven,
        },
    })
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
    let Some(effect) = classify_victory_or_defeat_life(definition, source_kind) else {
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
    let (post_round_effect, compact_effect) = match effect {
        VictoryOrDefeatLifeEffectV1::GainLife { life } => (
            CombatStatPostRoundEffectV1::GainLifeOnVictoryOrDefeat { life },
            CombatStatEffectV1::GainLifeOnVictoryOrDefeat { life },
        ),
        VictoryOrDefeatLifeEffectV1::ReduceOpponentLife { life, minimum } => (
            CombatStatPostRoundEffectV1::ReduceOpponentLifeOnVictoryOrDefeat { life, minimum },
            CombatStatEffectV1::ReduceOpponentLifeOnVictoryOrDefeat { life, minimum },
        ),
    };
    Ok(PreparedCatalogSourceV1 {
        metadata: CatalogCombatStatSourceDispositionV1::ExecutePostRound {
            identity: CatalogCombatStatModifierIdentityV1 {
                catalog_id,
                description: description.to_owned(),
                registry_definition_id: definition.id(),
                registry_alias_ids,
            },
            effect: post_round_effect,
        },
        compact: CombatStatSourcePlanV1::Execute {
            source_id: definition.id(),
            predicate: CombatStatPredicateV1::Always,
            effect: compact_effect,
        },
    })
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
    let Some((per_star, minimum)) =
        classify_equalizer_opponent_life_on_victory(definition, source_kind)
    else {
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
    Ok(PreparedCatalogSourceV1 {
        metadata: CatalogCombatStatSourceDispositionV1::ExecutePostRound {
            identity: CatalogCombatStatModifierIdentityV1 {
                catalog_id,
                description: description.to_owned(),
                registry_definition_id: definition.id(),
                registry_alias_ids,
            },
            effect: CombatStatPostRoundEffectV1::ReduceOpponentLifeOnVictoryPerOpponentStars {
                per_star,
                minimum,
            },
        },
        compact: CombatStatSourcePlanV1::Execute {
            source_id: definition.id(),
            predicate: CombatStatPredicateV1::Always,
            effect: CombatStatEffectV1::ReduceOpponentLifeOnVictoryPerOpponentStars {
                per_star,
                minimum,
            },
        },
    })
}

/// Catalog clan-bonus id 47 is not a capture registry id. It maps to the reviewed Riots
/// definition only after effective-clan activation. Printed ability ids map only to their
/// exact same registry definitions; notably, no catalog card can synthesize dynamic 1034.
/// Catalog authority for the two reviewed identities. Mou must be the exact card key and
/// printed ability id; Berzerk must be the active effective clan with its catalog bonus id.
/// Neither the registry's text lookup nor a card from another clan can substitute.
fn victory_opponent_life_registry_definition_id(
    card_key: CardKey,
    source_kind: CombatStatEffectSourceV1,
    effective_clan_id: u32,
    catalog_id: Option<u32>,
    description: &str,
) -> Option<u32> {
    match (source_kind, catalog_id) {
        (CombatStatEffectSourceV1::Ability, Some(MOU_VICTORY_OPPONENT_LIFE_REGISTRY_ID))
            if card_key == MOU_CARD && description == MOU_VICTORY_OPPONENT_LIFE_DESCRIPTION =>
        {
            Some(MOU_VICTORY_OPPONENT_LIFE_REGISTRY_ID)
        }
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
    let Some((life, minimum)) = classify_victory_opponent_life(definition, source_kind) else {
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
    Ok(PreparedCatalogSourceV1 {
        metadata: CatalogCombatStatSourceDispositionV1::ExecutePostRound {
            identity: CatalogCombatStatModifierIdentityV1 {
                catalog_id,
                description: description.to_owned(),
                registry_definition_id: definition.id(),
                registry_alias_ids,
            },
            effect: CombatStatPostRoundEffectV1::ReduceOpponentLifeOnVictory { life, minimum },
        },
        compact: CombatStatSourcePlanV1::Execute {
            source_id: definition.id(),
            predicate: CombatStatPredicateV1::Always,
            effect: CombatStatEffectV1::ReduceOpponentLifeOnVictory { life, minimum },
        },
    })
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
    if !classify_victory_or_defeat_pillz(definition, source_kind) {
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
    Ok(PreparedCatalogSourceV1 {
        metadata: CatalogCombatStatSourceDispositionV1::ExecutePostRound {
            identity: CatalogCombatStatModifierIdentityV1 {
                catalog_id,
                description: description.to_owned(),
                registry_definition_id: definition.id(),
                registry_alias_ids,
            },
            effect: CombatStatPostRoundEffectV1::GainOnePillzOnVictoryOrDefeat,
        },
        compact: CombatStatSourcePlanV1::Execute {
            source_id: definition.id(),
            predicate: CombatStatPredicateV1::Always,
            effect: CombatStatEffectV1::GainOnePillzOnVictoryOrDefeat,
        },
    })
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
    let definition = registry
        .lookup_capture(ANITA_COURAGE_DAMAGE_TO_LIFE_REGISTRY_ID, description)
        .map_err(|source| CatalogCombatStatMatchErrorV1::Lookup {
            player,
            hand_slot,
            source_kind,
            catalog_id,
            description: description.to_owned(),
            source,
        })?;
    if !classify_anita_courage_damage_to_life(definition, source_kind) {
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
    Ok(PreparedCatalogSourceV1 {
        metadata: CatalogCombatStatSourceDispositionV1::ExecutePostRound {
            identity: CatalogCombatStatModifierIdentityV1 {
                catalog_id,
                description: description.to_owned(),
                registry_definition_id: definition.id(),
                registry_alias_ids,
            },
            effect: CombatStatPostRoundEffectV1::GainLifeEqualToFinalDamageOnCourageVictory,
        },
        compact: CombatStatSourcePlanV1::Execute {
            source_id: definition.id(),
            predicate: CombatStatPredicateV1::OwnerMovesFirst,
            effect: CombatStatEffectV1::GainLifeEqualToFinalDamageOnCourageVictory,
        },
    })
}

fn prepare_komboka_victory_pillz_and_life_source(
    registry: &EffectRegistryV1,
    player: PlayerId,
    hand_slot: HandSlot,
    source_kind: CombatStatEffectSourceV1,
    catalog_id: Option<u32>,
    description: &str,
) -> Result<PreparedCatalogSourceV1, CatalogCombatStatMatchErrorV1> {
    let definition = registry
        .lookup_capture(KOMBOKA_VICTORY_PILLZ_AND_LIFE_REGISTRY_ID, description)
        .map_err(|source| CatalogCombatStatMatchErrorV1::Lookup {
            player,
            hand_slot,
            source_kind,
            catalog_id,
            description: description.to_owned(),
            source,
        })?;
    if !classify_komboka_victory_pillz_and_life(definition, source_kind) {
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
    Ok(PreparedCatalogSourceV1 {
        metadata: CatalogCombatStatSourceDispositionV1::ExecutePostRound {
            identity: CatalogCombatStatModifierIdentityV1 {
                catalog_id,
                description: description.to_owned(),
                registry_definition_id: definition.id(),
                registry_alias_ids,
            },
            effect: CombatStatPostRoundEffectV1::GainOnePillzAndLifeOnVictory,
        },
        compact: CombatStatSourcePlanV1::Execute {
            source_id: definition.id(),
            predicate: CombatStatPredicateV1::Always,
            effect: CombatStatEffectV1::GainOnePillzAndLifeOnVictory,
        },
    })
}
