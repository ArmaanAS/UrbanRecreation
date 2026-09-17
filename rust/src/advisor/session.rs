//! A small, committed-round state machine for the manual advisor.
//!
//! Search is deliberately not coupled to this module.  A caller can borrow the current
//! game for an advisory search, then submit the two observed selections here.  Unlike the
//! search path, a successful submission is intentionally retained: this is the boundary
//! between hypothetical make/unmake work and a real sequence of played rounds.

use std::error::Error;
use std::fmt;

use crate::engine::{
    BaseRulesRoundInput, BaseRulesRoundReport, BaseRulesSelection, ByPlayer,
    CombatStatDiagnosticErrorV1, CombatStatDiagnosticV1, MatchStatus, PlayerId,
};

/// Selection text is small in the actual UI, and this cap prevents a line-oriented host
/// from accidentally retaining or formatting arbitrarily large pasted input.
pub const MAX_SELECTION_TEXT_LEN: usize = 32;

/// One manually observed selection, in engine notation.
///
/// `pillz` excludes the free attack pill and, when `fury` is true, Fury's three-pill cost.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ManualSelection {
    pub hand_index: u8,
    pub pillz: u16,
    pub fury: bool,
}

impl ManualSelection {
    /// Parses `SLOT:PILLZ` or `SLOT:PILLZ:F` exactly.  Slots are zero based (`0..3`);
    /// legality against the current hand and pillz pool is checked by the engine at commit.
    pub fn parse(text: &str) -> Result<Self, ManualSelectionError> {
        if text.len() > MAX_SELECTION_TEXT_LEN {
            return Err(ManualSelectionError::TooLong {
                length: text.len(),
                maximum: MAX_SELECTION_TEXT_LEN,
            });
        }
        let text = text.trim();
        if text.is_empty() {
            return Err(ManualSelectionError::ExpectedSyntax);
        }
        let mut fields = text.split(':');
        let slot = fields.next().expect("non-empty input has a first field");
        let pillz = fields.next().ok_or(ManualSelectionError::ExpectedSyntax)?;
        let fury = match fields.next() {
            None => false,
            Some("F") => true,
            Some(_) => return Err(ManualSelectionError::InvalidFury),
        };
        if fields.next().is_some() {
            return Err(ManualSelectionError::ExpectedSyntax);
        }

        let hand_index = parse_decimal::<u8>(slot).ok_or(ManualSelectionError::InvalidSlot)?;
        if hand_index > 3 {
            return Err(ManualSelectionError::InvalidSlot);
        }
        let pillz = parse_decimal::<u16>(pillz).ok_or(ManualSelectionError::InvalidPillz)?;
        Ok(Self {
            hand_index,
            pillz,
            fury,
        })
    }

    pub const fn as_base_rules(self) -> BaseRulesSelection {
        BaseRulesSelection::new(self.hand_index, self.pillz, self.fury)
    }
}

/// A bounded, allocation-free selection parser's errors.  Rendering an error never copies
/// the untrusted input text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManualSelectionError {
    TooLong { length: usize, maximum: usize },
    ExpectedSyntax,
    InvalidSlot,
    InvalidPillz,
    InvalidFury,
}

impl fmt::Display for ManualSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLong { length, maximum } => write!(
                formatter,
                "selection is {length} bytes; at most {maximum} bytes are accepted"
            ),
            Self::ExpectedSyntax => write!(formatter, "selection must be SLOT:PILLZ[:F]"),
            Self::InvalidSlot => write!(formatter, "selection slot must be an integer in 0..3"),
            Self::InvalidPillz => write!(
                formatter,
                "selection pillz must be a non-negative 16-bit integer"
            ),
            Self::InvalidFury => write!(formatter, "selection Fury suffix must be uppercase F"),
        }
    }
}

impl Error for ManualSelectionError {}

/// Errors raised while turning observed manual input into a committed engine round.
#[derive(Debug)]
pub enum AdvisorSessionError {
    Selection(ManualSelectionError),
    Engine(CombatStatDiagnosticErrorV1),
}

impl fmt::Display for AdvisorSessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Selection(source) => source.fmt(formatter),
            Self::Engine(source) => write!(formatter, "cannot commit round: {source}"),
        }
    }
}

impl Error for AdvisorSessionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Selection(source) => Some(source),
            Self::Engine(source) => Some(source),
        }
    }
}

impl From<ManualSelectionError> for AdvisorSessionError {
    fn from(source: ManualSelectionError) -> Self {
        Self::Selection(source)
    }
}

/// A manually advanced diagnostic match.
///
/// The supplied `initial_first_mover` is explicit rather than inferred from round parity,
/// because imported positions can begin in any round.  After each successful committed
/// round, the next first mover alternates.  Failed submissions leave both game and mover
/// unchanged.
#[derive(Debug)]
pub struct AdvisorSession {
    game: CombatStatDiagnosticV1,
    next_first_mover: PlayerId,
}

impl AdvisorSession {
    pub fn new(game: CombatStatDiagnosticV1, initial_first_mover: PlayerId) -> Self {
        Self {
            game,
            next_first_mover: initial_first_mover,
        }
    }

    pub fn game(&self) -> &CombatStatDiagnosticV1 {
        &self.game
    }

    /// Temporarily borrows the current game for an advisory search.  The caller must leave
    /// it at the same position (the advisor search does so through make/unmake); committed
    /// moves belong in [`Self::commit_round`] instead.
    pub fn game_mut(&mut self) -> &mut CombatStatDiagnosticV1 {
        &mut self.game
    }

    pub const fn current_first_mover(&self) -> PlayerId {
        self.next_first_mover
    }

    pub fn round(&self) -> u8 {
        self.game.position().rounds_played
    }

    pub fn status(&self) -> MatchStatus {
        self.game.position().status
    }

    /// Parses and commits one complete observed round.  The two arguments are always P1
    /// then P2; move order is supplied by the session's explicit current first mover.
    pub fn commit_text_round(
        &mut self,
        p1: &str,
        p2: &str,
    ) -> Result<BaseRulesRoundReport, AdvisorSessionError> {
        self.commit_round(ByPlayer::new(
            ManualSelection::parse(p1)?,
            ManualSelection::parse(p2)?,
        ))
    }

    /// Commits one complete observed round through the real diagnostic engine.
    pub fn commit_round(
        &mut self,
        selections: ByPlayer<ManualSelection>,
    ) -> Result<BaseRulesRoundReport, AdvisorSessionError> {
        let input = BaseRulesRoundInput {
            first_mover: self.next_first_mover,
            selections: selections.map(ManualSelection::as_base_rules),
        };
        let (report, _undo) = self.game.make(input).map_err(AdvisorSessionError::Engine)?;
        self.next_first_mover = self.next_first_mover.other();
        Ok(report)
    }
}

fn parse_decimal<T>(text: &str) -> Option<T>
where
    T: std::str::FromStr,
{
    (!text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| text.parse::<T>().ok())
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::CardKey;
    use crate::engine::{
        BaseRulesCardSpec, BaseRulesMatchSpec, BaseRulesPlayerSpec, CombatStatCardPlanV1,
        CombatStatDiagnosticMatchSpecV1, CombatStatSourcePlanV1,
    };

    fn test_game() -> CombatStatDiagnosticV1 {
        let card = |id| BaseRulesCardSpec {
            key: CardKey::new(id, 1),
            clan_id: id,
            power: 5,
            damage: 1,
        };
        let base_rules = BaseRulesMatchSpec {
            battle_rule_id: 0,
            night: false,
            players: ByPlayer::new(
                BaseRulesPlayerSpec {
                    initial_life: 20,
                    initial_pillz: 3,
                    hand: std::array::from_fn(|slot| card(100 + slot as u32)),
                },
                BaseRulesPlayerSpec {
                    initial_life: 20,
                    initial_pillz: 3,
                    hand: std::array::from_fn(|slot| card(200 + slot as u32)),
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
        CombatStatDiagnosticV1::new(CombatStatDiagnosticMatchSpecV1 { base_rules, cards })
            .expect("effect-free test match is supported")
    }

    #[test]
    fn parses_the_compact_selection_syntax_without_permissive_variants() {
        assert_eq!(
            ManualSelection::parse(" 2:11:F ").unwrap(),
            ManualSelection {
                hand_index: 2,
                pillz: 11,
                fury: true,
            }
        );
        assert_eq!(ManualSelection::parse("0:0").unwrap().fury, false);
        for text in ["", "0", "4:0", "0:-1", "0:1:f", "0:1:F:extra", "+0:1"] {
            assert!(ManualSelection::parse(text).is_err(), "{text:?} must fail");
        }
    }

    #[test]
    fn parser_bounds_untrusted_text_before_formatting_it() {
        let too_long = "0:0".repeat(11);
        assert_eq!(
            ManualSelection::parse(&too_long),
            Err(ManualSelectionError::TooLong {
                length: 33,
                maximum: MAX_SELECTION_TEXT_LEN,
            })
        );
    }

    #[test]
    fn committed_rounds_alternate_the_explicit_mover_and_preserve_failures() {
        let mut session = AdvisorSession::new(test_game(), PlayerId::P2);
        assert_eq!(session.round(), 0);
        assert_eq!(session.status(), MatchStatus::Playing);
        assert_eq!(session.current_first_mover(), PlayerId::P2);

        let report = session.commit_text_round("0:0", "0:0").unwrap();
        assert_eq!(report.first_mover, PlayerId::P2);
        assert_eq!(session.round(), 1);
        assert_eq!(session.current_first_mover(), PlayerId::P1);
        assert!(session.game().position().played[PlayerId::P1][0]);

        let before = session.game().clone();
        let error = session.commit_text_round("0:0", "1:0").unwrap_err();
        assert!(matches!(error, AdvisorSessionError::Engine(_)));
        assert_eq!(session.game(), &before);
        assert_eq!(session.round(), 1);
        assert_eq!(session.current_first_mover(), PlayerId::P1);

        let report = session.commit_text_round("1:0", "1:0").unwrap();
        assert_eq!(report.first_mover, PlayerId::P1);
        assert_eq!(session.round(), 2);
        assert_eq!(session.current_first_mover(), PlayerId::P2);
    }
}
