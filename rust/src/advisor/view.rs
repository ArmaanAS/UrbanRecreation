//! Pure terminal rendering for the bounded Rust advisor.
//!
//! This module deliberately has no terminal I/O. The binary supplies explicit dimensions
//! and chooses whether to emit ANSI; tests and non-interactive callers render the same frame.

use super::search::{RankedMove, SearchSnapshot};
use crate::engine::PlayerId;

/// One visible card in an advisor hand.  This is presentation data, not an engine card.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdvisorCard {
    pub name: String,
    pub id: u32,
    pub level: u8,
    pub power: u16,
    pub damage: u16,
    pub played: bool,
}

/// One side of the advisor board.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdvisorSide {
    pub label: String,
    pub life: u16,
    pub pillz: u16,
    pub cards: [AdvisorCard; 4],
}

/// All data needed to paint an advisor frame.
///
/// `p1` and `p2` intentionally remain in engine order.  `us` determines which one is
/// labelled "YOU" in the view, without losing the unambiguous P1/P2 identity needed when
/// reading a capture or a solver trace.
#[derive(Clone, Debug)]
pub struct AdvisorViewModel {
    pub p1: AdvisorSide,
    pub p2: AdvisorSide,
    pub us: PlayerId,
    /// Such as `EXACT OPENING POLICY` or `EXACT CONTINUATION POLICY`; supplied by the caller so
    /// solver modes do not have to fork the renderer.
    pub mode: String,
    /// A short, visible statement of what this solver does not yet model.
    pub limitation: String,
    pub snapshot: SearchSnapshot,
    /// Number of the eight cards whose complete semantics were admitted by the strict
    /// catalog boundary.
    pub supported_cards: usize,
    pub provenance_revision: u16,
}

/// Whether the returned frame contains the standard ANSI sixteen-colour palette.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColourMode {
    Always,
    Never,
}

const ESC: &str = "\x1b[";
const RESET: &str = "\x1b[0m";

const CYAN: u8 = 96;
const GREEN: u8 = 92;
const YELLOW: u8 = 93;
const RED: u8 = 91;
const GREY: u8 = 90;
const BLUE: u8 = 94;
const MAGENTA: u8 = 95;

/// Render a bounded, self-contained advisor frame.
///
/// No returned line contains more than `width` visible Unicode scalar values, and the
/// returned frame contains no more than `height` lines. Wide glyphs can occupy more than
/// one terminal cell. A zero-sized terminal has no representable frame and therefore
/// returns an empty string. Untrusted display text is reduced to one printable line before
/// rendering; the renderer itself only emits simple standard SGR colours, never cursor
/// controls or the 256-colour palette.
pub fn render(model: &AdvisorViewModel, width: usize, height: usize, colour: ColourMode) -> String {
    if width == 0 || height == 0 {
        return String::new();
    }

    let compact = width < 60 || height <= 14;
    let us = side(model, model.us);
    let them = side(model, model.us.other());
    let us_id = player_name(model.us);
    let them_id = player_name(model.us.other());
    let mut lines = Vec::new();

    lines.push(style("RUST ADVISOR · COMBAT-STAT V1", CYAN, true, colour));
    let mode = if model.mode.trim().is_empty() {
        "EXACT POLICY".to_owned()
    } else {
        display_text(model.mode.trim())
    };
    lines.push(format!(
        "{} {}",
        style("MODE", GREY, false, colour),
        style(&mode, YELLOW, true, colour),
    ));
    lines.push(format!(
        "{} {}",
        style("LIMIT", GREY, false, colour),
        display_text(model.limitation.trim())
    ));

    if compact {
        lines.push(compact_side("YOU", us_id, us, colour));
        lines.push(compact_side("THEM", them_id, them, colour));
    } else {
        lines.push(String::new());
        lines.push(side_summary("YOU", us_id, us, colour));
        lines.push(hand_line(us, colour));
        lines.push(side_summary("THEM", them_id, them, colour));
        lines.push(hand_line(them, colour));
        lines.push(String::new());
    }

    lines.push(table_heading(width, colour));
    let reserve = 2; // progress and provenance stay visible at every practical height.
    let available_rows = height.saturating_sub(lines.len() + reserve);
    let desired_rows = if compact { 2 } else { 8 };
    let rows = available_rows.min(desired_rows);
    if rows == 0 && height > lines.len() {
        lines.push(style("no room for recommendations", GREY, false, colour));
    } else if model.snapshot.ranked.is_empty() {
        lines.push(style("  no legal moves evaluated", GREY, false, colour));
    } else {
        for (rank, move_) in model.snapshot.ranked.iter().take(rows).enumerate() {
            lines.push(move_row(rank + 1, move_, us, width, colour));
        }
    }

    lines.push(progress_line(&model.snapshot, colour));
    lines.push(format!(
        "{} {}/8 · rev {}",
        style("PROVENANCE", GREY, false, colour),
        model.supported_cards,
        model.provenance_revision,
    ));

    lines
        .into_iter()
        .take(height)
        .map(|line| clip_ansi(&line, width, colour))
        .collect::<Vec<_>>()
        .join("\n")
}

fn side<'a>(model: &'a AdvisorViewModel, player: PlayerId) -> &'a AdvisorSide {
    match player {
        PlayerId::P1 => &model.p1,
        PlayerId::P2 => &model.p2,
    }
}

fn player_name(player: PlayerId) -> &'static str {
    match player {
        PlayerId::P1 => "P1",
        PlayerId::P2 => "P2",
    }
}

fn compact_side(role: &str, player: &str, side: &AdvisorSide, colour: ColourMode) -> String {
    let label = side_label(side);
    format!(
        "{} {}{}  L{} P{}  {}",
        style(role, BLUE, true, colour),
        style(player, GREY, false, colour),
        label,
        side.life,
        side.pillz,
        cards_compact(side, colour),
    )
}

fn side_summary(role: &str, player: &str, side: &AdvisorSide, colour: ColourMode) -> String {
    let label = side_label(side);
    format!(
        "{} {}{}  {} {}  {} {}",
        style(role, BLUE, true, colour),
        style(player, GREY, false, colour),
        label,
        style("LIFE", GREY, false, colour),
        style(&side.life.to_string(), GREEN, true, colour),
        style("PILLZ", GREY, false, colour),
        style(&side.pillz.to_string(), CYAN, true, colour),
    )
}

fn side_label(side: &AdvisorSide) -> String {
    let label = display_text(side.label.trim());
    if label.is_empty() {
        String::new()
    } else {
        format!(" · {label}")
    }
}

fn hand_line(side: &AdvisorSide, colour: ColourMode) -> String {
    format!("  {}", cards_compact(side, colour))
}

fn cards_compact(side: &AdvisorSide, colour: ColourMode) -> String {
    side.cards
        .iter()
        .enumerate()
        .map(|(index, card)| {
            let mark = if card.played { "×" } else { "·" };
            let name = if card.name.is_empty() {
                "(unnamed)".to_owned()
            } else {
                display_text(&card.name)
            };
            let text = format!("{index}:{name} {}/{}{}", card.power, card.damage, mark);
            if card.played {
                style(&text, GREY, false, colour)
            } else {
                text
            }
        })
        .collect::<Vec<_>>()
        .join("  ")
}

fn table_heading(width: usize, colour: ColourMode) -> String {
    let text = if width >= 104 {
        " #  CARD                         BET       AVG  WORST    KO  RISK  SAMPLES"
    } else if width >= 72 {
        " #  CARD                     BET       AVG  WORST    KO/RISK  SAMPLES"
    } else {
        " #  CARD                 BET     AVG  WORST  PROGRESS"
    };
    style(text, GREY, false, colour)
}

fn move_row(
    rank: usize,
    move_: &RankedMove,
    side: &AdvisorSide,
    width: usize,
    colour: ColourMode,
) -> String {
    let slot = usize::from(move_.move_.hand_index);
    let card = side.cards.get(slot);
    let raw_name = card
        .map(|card| card.name.as_str())
        .unwrap_or("invalid hand slot");
    let name_width = if width >= 104 {
        28
    } else if width >= 72 {
        22
    } else {
        18
    };
    let card_name = fit_plain(&display_text(raw_name), name_width);
    let bet = if move_.move_.fury {
        format!("{} +F", move_.move_.pillz)
    } else {
        move_.move_.pillz.to_string()
    };
    let avg = displayed_score(move_.average);
    let worst = displayed_score(move_.worst);
    let risk = if move_.samples == 0 {
        "-".to_string()
    } else {
        format!("{}/{}", move_.kos, move_.koed)
    };
    let progress = if move_.samples == 0 {
        "waiting".to_string()
    } else {
        format!("{} samples", move_.samples)
    };
    let average_colour = score_colour(move_.average);
    let worst_colour = score_colour(move_.worst);
    let bet_colour = if move_.move_.fury { MAGENTA } else { CYAN };

    if width >= 104 {
        let bet = pad_left(&bet, 7);
        let avg = pad_left(&avg, 4);
        let worst = pad_left(&worst, 5);
        format!(
            " {rank:>1}.  {card_name:<28} {}  {}  {}  {:>4}  {:>4}  {:>7}",
            style(&bet, bet_colour, move_.move_.fury, colour),
            style(&avg, average_colour, true, colour),
            style(&worst, worst_colour, false, colour),
            move_.kos,
            move_.koed,
            move_.samples,
        )
    } else if width >= 72 {
        let bet = pad_left(&bet, 7);
        let avg = pad_left(&avg, 4);
        let worst = pad_left(&worst, 5);
        format!(
            " {rank:>1}.  {card_name:<22} {}  {}  {}  {:>7}  {:>7}",
            style(&bet, bet_colour, move_.move_.fury, colour),
            style(&avg, average_colour, true, colour),
            style(&worst, worst_colour, false, colour),
            risk,
            move_.samples,
        )
    } else {
        let bet = pad_left(&bet, 5);
        let avg = pad_left(&avg, 4);
        let worst = pad_left(&worst, 5);
        format!(
            " {rank:>1}.  {card_name:<18} {}  {}  {}  {}",
            style(&bet, bet_colour, move_.move_.fury, colour),
            style(&avg, average_colour, true, colour),
            style(&worst, worst_colour, false, colour),
            progress,
        )
    }
}

fn pad_left(text: &str, width: usize) -> String {
    format!("{:>width$}", text)
}

fn progress_line(snapshot: &SearchSnapshot, colour: ColourMode) -> String {
    let state = if snapshot.complete {
        "COMPLETE"
    } else {
        "PARTIAL"
    };
    let state_colour = if snapshot.complete { GREEN } else { YELLOW };
    format!(
        "{} {}  {}/{} units · {}",
        style("SEARCH", GREY, false, colour),
        style(state, state_colour, true, colour),
        snapshot.units_done,
        snapshot.units_total,
        duration(snapshot.elapsed),
    )
}

/// The solver's values are outcomes in the asking player's frame in [-1, 1], shown as a
/// win percentage. A rounded near-win or near-loss never displays as an exact endpoint.
fn displayed_score(value: f64) -> String {
    if !value.is_finite() {
        return "--".to_owned();
    }
    let percent = ((value.clamp(-1.0, 1.0) + 1.0) * 50.0).round() as i16;
    let percent = if percent >= 100 && value < 1.0 {
        99
    } else if percent <= 0 && value > -1.0 {
        1
    } else {
        percent
    };
    format!("{percent}%")
}

fn score_colour(value: f64) -> u8 {
    if !value.is_finite() {
        GREY
    } else if value >= 0.6 {
        GREEN
    } else if value >= -0.1 {
        YELLOW
    } else {
        RED
    }
}

fn duration(duration: std::time::Duration) -> String {
    if duration.as_secs() == 0 {
        format!("{}ms", duration.as_millis())
    } else {
        format!("{:.1}s", duration.as_secs_f64())
    }
}

fn style(text: &str, colour_code: u8, bold: bool, mode: ColourMode) -> String {
    if mode == ColourMode::Never {
        return text.to_string();
    }
    if bold {
        format!("{ESC}1;{colour_code}m{text}{RESET}")
    } else {
        format!("{ESC}{colour_code}m{text}{RESET}")
    }
}

fn display_text(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character.is_control() {
                '?'
            } else {
                character
            }
        })
        .collect()
}

fn fit_plain(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    if width <= 1 {
        return "…".chars().take(width).collect();
    }
    let mut out = text.chars().take(width - 1).collect::<String>();
    out.push('…');
    out
}

/// Cut a formatted line to a printable width without splitting an ANSI escape sequence.
fn clip_ansi(line: &str, width: usize, colour: ColourMode) -> String {
    if colour == ColourMode::Never {
        return fit_plain(line, width);
    }
    let mut output = String::new();
    let mut chars = line.chars().peekable();
    let mut used = 0usize;
    let mut saw_sgr = false;
    while let Some(character) = chars.next() {
        if character == '\x1b' && chars.peek() == Some(&'[') {
            output.push(character);
            output.push(chars.next().expect("escape introducer is present"));
            for control in chars.by_ref() {
                output.push(control);
                if ('@'..='~').contains(&control) {
                    if control == 'm' {
                        saw_sgr = true;
                    }
                    break;
                }
            }
            continue;
        }
        if used >= width {
            break;
        }
        output.push(character);
        used += 1;
    }
    if saw_sgr && !output.ends_with(RESET) {
        output.push_str(RESET);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::advisor::search::{AdvisorMove, EvaluationKind, RankedMove, SearchSnapshot};
    use std::time::Duration;

    fn card(name: &str, played: bool) -> AdvisorCard {
        AdvisorCard {
            name: name.to_string(),
            id: 1,
            level: 1,
            power: 7,
            damage: 4,
            played,
        }
    }

    fn snapshot(complete: bool) -> SearchSnapshot {
        SearchSnapshot {
            ranked: vec![
                RankedMove {
                    move_: AdvisorMove {
                        hand_index: 0,
                        pillz: 4,
                        fury: false,
                    },
                    average: 0.6,
                    worst: -1.0,
                    best: 1.0,
                    samples: 12,
                    kos: 3,
                    koed: 1,
                    hidden_outcomes: Vec::new(),
                },
                RankedMove {
                    move_: AdvisorMove {
                        hand_index: 2,
                        pillz: 1,
                        fury: true,
                    },
                    average: 0.0,
                    worst: -0.2,
                    best: 0.4,
                    samples: 4,
                    kos: 0,
                    koed: 2,
                    hidden_outcomes: Vec::new(),
                },
            ],
            units_done: if complete { 16 } else { 4 },
            units_total: 16,
            elapsed: Duration::from_millis(12),
            complete,
            evaluation: EvaluationKind::ExactOpeningPolicy,
        }
    }

    fn model(complete: bool) -> AdvisorViewModel {
        AdvisorViewModel {
            p1: AdvisorSide {
                label: "Alice".to_string(),
                life: 12,
                pillz: 8,
                cards: [
                    card("Alpha", false),
                    card("Bravo", false),
                    card("Cobra", true),
                    card("Delta", false),
                ],
            },
            p2: AdvisorSide {
                label: "Bob".to_string(),
                life: 9,
                pillz: 5,
                cards: [
                    card("Echo", false),
                    card("Foxtrot", true),
                    card("Gamma", false),
                    card("Hotel", false),
                ],
            },
            us: PlayerId::P1,
            mode: "EXACT OPENING POLICY".to_string(),
            limitation: "weighted captured replies; supported effects only".to_string(),
            snapshot: snapshot(complete),
            supported_cards: 8,
            provenance_revision: 10,
        }
    }

    fn no_ansi(text: &str) -> String {
        let mut output = String::new();
        let mut chars = text.chars().peekable();
        while let Some(character) = chars.next() {
            if character == '\x1b' && chars.peek() == Some(&'[') {
                chars.next();
                for control in chars.by_ref() {
                    if ('@'..='~').contains(&control) {
                        break;
                    }
                }
            } else {
                output.push(character);
            }
        }
        output
    }

    #[test]
    fn plain_frame_is_deterministic_and_informative() {
        let view = render(&model(true), 80, 24, ColourMode::Never);
        assert_eq!(
            view,
            "RUST ADVISOR · COMBAT-STAT V1\nMODE EXACT OPENING POLICY\nLIMIT weighted captured replies; supported effects only\n\nYOU P1 · Alice  LIFE 12  PILLZ 8\n  0:Alpha 7/4·  1:Bravo 7/4·  2:Cobra 7/4×  3:Delta 7/4·\nTHEM P2 · Bob  LIFE 9  PILLZ 5\n  0:Echo 7/4·  1:Foxtrot 7/4×  2:Gamma 7/4·  3:Hotel 7/4·\n\n #  CARD                     BET       AVG  WORST    KO/RISK  SAMPLES\n 1.  Alpha                        4   80%     0%      3/1       12\n 2.  Cobra                     1 +F   50%    40%      0/2        4\nSEARCH COMPLETE  16/16 units · 12ms\nPROVENANCE 8/8 · rev 10"
        );
    }

    #[test]
    fn frame_respects_small_and_large_terminal_bounds() {
        for (width, height) in [(40, 10), (80, 24), (132, 40)] {
            let frame = render(&model(false), width, height, ColourMode::Never);
            let lines = frame.lines().collect::<Vec<_>>();
            assert!(lines.len() <= height);
            assert!(lines.iter().all(|line| line.chars().count() <= width));
            assert!(frame.contains("RUST ADVISOR"));
            assert!(frame.contains("PARTIAL"));
        }
    }

    #[test]
    fn ansi_mode_has_the_same_visible_frame_as_plain_mode() {
        let model = model(false);
        let plain = render(&model, 80, 24, ColourMode::Never);
        let coloured = render(&model, 80, 24, ColourMode::Always);
        assert!(coloured.contains("\x1b["));
        assert_eq!(no_ansi(&coloured), plain);
        assert!(plain.is_ascii() || !plain.contains('\x1b'));
    }

    #[test]
    fn empty_snapshot_announces_waiting_without_ansi_when_disabled() {
        let mut model = model(false);
        model.snapshot.ranked.clear();
        let frame = render(&model, 40, 10, ColourMode::Never);
        assert!(frame.contains("no legal moves evaluated"));
        assert!(!frame.contains('\x1b'));
    }

    #[test]
    fn unevaluated_moves_never_look_like_zero_percent_losses() {
        let mut model = model(false);
        model.snapshot.ranked[0].average = f64::NAN;
        model.snapshot.ranked[0].worst = f64::NAN;
        model.snapshot.ranked[0].best = f64::NAN;
        model.snapshot.ranked[0].samples = 0;
        let frame = render(&model, 80, 24, ColourMode::Never);
        let line = frame
            .lines()
            .find(|line| line.contains("1.  Alpha"))
            .expect("the unevaluated move remains visible");
        assert!(line.matches("--").count() >= 2);
        assert!(!line.contains("0%"));
    }

    #[test]
    fn the_opening_and_later_rounds_show_the_same_win_and_worst_columns() {
        let opening = render(&model(true), 80, 24, ColourMode::Never);
        let mut later = model(true);
        later.snapshot.evaluation = EvaluationKind::ExactContinuationPolicy;
        later.mode = "EXACT CONTINUATION POLICY".to_owned();
        let later = render(&later, 80, 24, ColourMode::Never);
        for frame in [opening, later] {
            assert!(frame.contains("AVG  WORST"));
            assert!(frame.contains("80%"));
            assert!(frame.contains("0%"));
            assert!(!frame.contains("RANGE"));
        }
    }

    #[test]
    fn percentages_clamp_nonterminal_endpoints() {
        assert_eq!(displayed_score(0.999), "99%");
        assert_eq!(displayed_score(-0.999), "1%");
        assert_eq!(displayed_score(1.0), "100%");
        assert_eq!(displayed_score(-1.0), "0%");
    }

    #[test]
    fn catalog_text_cannot_inject_terminal_controls_or_extra_lines() {
        let mut model = model(false);
        model.p1.cards[0].name = "bad\x1b[31m\nname".to_owned();
        model.p1.label = "side\rlabel".to_owned();
        model.mode = "mode\tname".to_owned();
        model.limitation = "limit\nline".to_owned();

        let plain = render(&model, 80, 24, ColourMode::Never);
        assert!(plain
            .chars()
            .all(|character| character == '\n' || !character.is_control()));
        assert!(plain.contains("bad?[31m?name"));
        assert!(plain.contains("side?label"));
        assert!(plain.contains("mode?name"));
        assert!(plain.contains("limit?line"));

        let coloured = render(&model, 80, 24, ColourMode::Always);
        assert_eq!(no_ansi(&coloured), plain);
    }
}
