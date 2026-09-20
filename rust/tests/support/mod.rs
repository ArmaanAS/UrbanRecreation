//! Regenerable expectation files for the derived pins the gates carry.
//!
//! A pin like the combat-stat gate's executed-source set is not written by hand: it is
//! whatever the projection did over the fixtures, and every landed slice moves it. Keeping
//! it in `rust/tests/expect/<name>.txt` means a slice is recorded by regenerating the file
//! and reading the diff, which is also the honest unlock evidence - the diff names the ids
//! and draws the slice added, instead of a count copied into a commit message by hand.
//!
//! ```bash
//! UR_UPDATE_EXPECT=1 cargo test --manifest-path rust/Cargo.toml --locked
//! git diff rust/tests/expect   # review before committing; this is the slice's evidence
//! ```
//!
//! Regeneration never happens implicitly: without the variable a moved pin fails, and with
//! it every changed file is printed. Always read the diff - an unlock you did not predict
//! is a finding, not a formality.

#![allow(dead_code)]

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

const UPDATE_ENV: &str = "UR_UPDATE_EXPECT";
const WRAP_COLUMNS: usize = 96;

fn updating() -> bool {
    std::env::var_os(UPDATE_ENV).is_some_and(|value| !value.is_empty() && value != "0")
}

fn expect_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("expect")
        .join(format!("{name}.txt"))
}

fn header(name: &str, description: &str) -> String {
    format!(
        "# {name}\n\
         # {description}\n\
         # Derived, not hand-written. Regenerate with:\n\
         #   UR_UPDATE_EXPECT=1 cargo test --manifest-path rust/Cargo.toml --locked\n\
         # Then read `git diff` on this file: it is the slice's unlock evidence.\n"
    )
}

fn read_body(name: &str) -> Option<String> {
    let text = fs::read_to_string(expect_path(name)).ok()?;
    Some(
        text.lines()
            .filter(|line| !line.trim_start().starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn write_file(name: &str, description: &str, body: &str) {
    let path = expect_path(name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|error| panic!("create {}: {error}", parent.display()));
    }
    let contents = format!("{}{}\n", header(name, description), body.trim_end());
    fs::write(&path, contents).unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
}

fn parse_numbers(body: &str) -> BTreeSet<u64> {
    body.split(|character: char| character.is_whitespace() || character == ',')
        .filter(|token| !token.is_empty())
        .map(|token| {
            token
                .parse::<u64>()
                .unwrap_or_else(|error| panic!("expectation file token {token:?}: {error}"))
        })
        .collect()
}

fn render_numbers(values: &BTreeSet<u64>) -> String {
    let mut rendered = String::new();
    let mut line = String::new();
    for value in values {
        let token = format!("{value},");
        if !line.is_empty() && line.len() + 1 + token.len() > WRAP_COLUMNS {
            rendered.push_str(line.trim_end());
            rendered.push('\n');
            line.clear();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(&token);
    }
    if !line.is_empty() {
        rendered.push_str(line.trim_end());
        rendered.push('\n');
    }
    rendered
}

fn describe_move(added: &[u64], removed: &[u64]) -> String {
    let mut report = String::new();
    if !added.is_empty() {
        writeln!(report, "  added ({}): {:?}", added.len(), added).unwrap();
    }
    if !removed.is_empty() {
        writeln!(report, "  removed ({}): {:?}", removed.len(), removed).unwrap();
    }
    report
}

/// Pin a derived set of ids - executed sources, disabled sources, eligible draws.
pub fn expect_ids<T: Copy + Into<u64>>(name: &str, description: &str, actual: &BTreeSet<T>) {
    let actual: &BTreeSet<u64> = &actual.iter().copied().map(Into::into).collect();
    let body = render_numbers(actual);
    let Some(stored) = read_body(name) else {
        if updating() {
            write_file(name, description, &body);
            eprintln!("{name}: created with {} ids", actual.len());
            return;
        }
        panic!(
            "expectation file {} is missing; regenerate with {UPDATE_ENV}=1",
            expect_path(name).display()
        );
    };
    let expected = parse_numbers(&stored);
    if &expected == actual {
        return;
    }
    let added: Vec<u64> = actual.difference(&expected).copied().collect();
    let removed: Vec<u64> = expected.difference(actual).copied().collect();
    if updating() {
        write_file(name, description, &body);
        eprintln!(
            "{name}: {} -> {} ids\n{}",
            expected.len(),
            actual.len(),
            describe_move(&added, &removed)
        );
        return;
    }
    panic!(
        "{name} moved: {} -> {} ids\n{}\
         Regenerate with {UPDATE_ENV}=1 and review the diff before committing.",
        expected.len(),
        actual.len(),
        describe_move(&added, &removed)
    );
}

/// Pin a derived count - gate rounds, scanned draws, absent dispositions.
pub fn expect_count(name: &str, description: &str, actual: usize) {
    let body = format!("{actual}\n");
    let Some(stored) = read_body(name) else {
        if updating() {
            write_file(name, description, &body);
            eprintln!("{name}: created as {actual}");
            return;
        }
        panic!(
            "expectation file {} is missing; regenerate with {UPDATE_ENV}=1",
            expect_path(name).display()
        );
    };
    let expected = parse_numbers(&stored);
    let expected = match expected.len() {
        1 => *expected.iter().next().unwrap() as usize,
        other => panic!("expectation file {name} holds {other} values, expected exactly one"),
    };
    if expected == actual {
        return;
    }
    if updating() {
        write_file(name, description, &body);
        eprintln!("{name}: {expected} -> {actual}");
        return;
    }
    panic!(
        "{name} moved: {expected} -> {actual}. \
         Regenerate with {UPDATE_ENV}=1 and review the diff before committing."
    );
}
