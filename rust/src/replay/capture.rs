use serde::Deserialize;
use std::io::Read;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapturedGame {
    pub(crate) id: u64,
    pub(crate) captured_at: String,
    pub(crate) creation_time: u64,
    pub(crate) room: Option<CapturedRoom>,
    pub(crate) battle_rule_id: u32,
    pub(crate) night: bool,
    pub(crate) my_id: Option<u64>,
    pub(crate) my_side: Option<i64>,
    pub(crate) first_player: Option<i64>,
    pub(crate) players: Vec<CapturedPlayer>,
    pub(crate) rounds: Vec<CapturedRound>,
    pub(crate) final_status: String,
    pub(crate) snapshots: u32,
    #[serde(default)]
    pub(crate) issues: Vec<String>,
    pub(crate) testcase: Option<CapturedTestcase>,
}

impl CapturedGame {
    pub fn from_reader(reader: impl Read) -> Result<Self, serde_json::Error> {
        serde_json::from_reader(reader)
    }

    pub const fn battle_id(&self) -> u64 {
        self.id
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CapturedRoom {
    pub(crate) id: u64,
    pub(crate) name: String,
    pub(crate) id_battle_rule: u32,
    pub(crate) id_deck_format: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CapturedPlayer {
    pub(crate) side: i64,
    pub(crate) id: u64,
    pub(crate) name: String,
    pub(crate) level: i64,
    pub(crate) grade: String,
    pub(crate) country: String,
    pub(crate) club: Option<CapturedClub>,
    pub(crate) registration_time: u64,
    pub(crate) certification_level: i64,
    pub(crate) base_life: i64,
    pub(crate) base_pillz: i64,
    pub(crate) hand: Vec<CapturedCard>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct CapturedClub {
    pub(crate) id: u64,
    pub(crate) name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CapturedCard {
    pub(crate) id: u32,
    pub(crate) name: Option<String>,
    pub(crate) clan: String,
    pub(crate) level: i64,
    pub(crate) index: i64,
    pub(crate) in_battle_id: u32,
    pub(crate) state: String,
    pub(crate) ability: Option<CapturedModifier>,
    pub(crate) bonus: Option<CapturedModifier>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct CapturedModifier {
    pub(crate) id: u32,
    pub(crate) description: String,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct CapturedRound {
    pub(crate) round: i64,
    pub(crate) first: Option<i64>,
    pub(crate) moves: Vec<CapturedMove>,
    pub(crate) resolution: [Option<CapturedResolution>; 2],
    pub(crate) life: [Option<i64>; 2],
    pub(crate) pillz: [Option<i64>; 2],
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CapturedMove {
    pub(crate) side: i64,
    pub(crate) index: i64,
    pub(crate) card_id: u32,
    pub(crate) pillz: i64,
    pub(crate) pillz_used: i64,
    pub(crate) fury: bool,
    pub(crate) t: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CapturedResolution {
    pub(crate) power: i64,
    pub(crate) damage: i64,
    #[allow(dead_code)]
    pub(crate) damage_after: i64,
    pub(crate) attack: i64,
    pub(crate) won: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct CapturedTestcase {
    pub(crate) cards: Vec<String>,
    pub(crate) levels: Vec<i64>,
    pub(crate) night: bool,
    pub(crate) flip: bool,
    pub(crate) life: i64,
    pub(crate) pillz: i64,
    pub(crate) moves: Vec<CapturedTestcaseMove>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct CapturedTestcaseMove {
    pub(crate) s1: (i64, i64, bool),
    pub(crate) s2: (i64, i64, bool),
    pub(crate) p1life: i64,
    pub(crate) p2life: i64,
    pub(crate) p1pillz: i64,
    pub(crate) p2pillz: i64,
    pub(crate) r1: Option<CapturedTestcaseResult>,
    pub(crate) r2: Option<CapturedTestcaseResult>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct CapturedTestcaseResult {
    pub(crate) power: i64,
    pub(crate) damage: i64,
    pub(crate) attack: i64,
    pub(crate) won: bool,
}
