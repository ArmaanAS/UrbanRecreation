pub mod adapter;
pub mod capture;
mod clan_bonus_diagnostic;
mod combat_stat_diagnostic;
pub mod corpus;
pub mod execute;
pub mod model;

pub use adapter::{
    adapt_capture, AdapterError, AdapterErrorKind, ReplayClassification, ReplaySkipReason,
    SkippedReplay,
};
pub use capture::CapturedGame;
pub use clan_bonus_diagnostic::{
    ClanBonusDiagnosticPreparationErrorV1, ClanBonusDiagnosticProjectionV1,
    ClanBonusDiagnosticProvenanceV1, ClanBonusDiagnosticReplayErrorV1,
    ClanBonusDiagnosticReplayReportV1, ClanBonusDiagnosticReplayV1,
    ClanBonusDiagnosticRoundReportV1, ClanBonusDiagnosticSelectedCardReportV1,
    DiagnosticCardPreparationV1, DiagnosticDisabledReasonV1, DiagnosticModifierIdentityV1,
    DiagnosticProjectionDispositionV1, DiagnosticReplayModelV1,
};
pub use combat_stat_diagnostic::{
    CombatStatCardPreparationV1, CombatStatDiagnosticPreparationErrorV1,
    CombatStatDiagnosticProjectionV1, CombatStatDiagnosticProvenanceV1,
    CombatStatDiagnosticReplayErrorV1, CombatStatDiagnosticReplayReportV1,
    CombatStatDiagnosticReplayV1, CombatStatDiagnosticRoundReportV1,
    CombatStatDiagnosticSelectedCardReportV1, CombatStatDisabledReasonV1,
    CombatStatModifierIdentityV1, CombatStatProjectionDispositionV1, CombatStatReplayModelV1,
    CombatStatWholeHandHazardSourceV1, COMBAT_STAT_DIAGNOSTIC_COMPILER_POLICY_SEMANTIC_REVISION_V1,
};
pub use corpus::{load_corpus, CorpusEntryError, CorpusErrorKind, CorpusLoadError, ReplayCorpus};
pub use execute::{
    BaseRulesReplay, BaseRulesReplayError, BaseRulesReplayReport, ReplayValidationError,
};
pub use model::*;
