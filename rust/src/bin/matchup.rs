//! Batch JSONL hand-versus-hand solver for deck evaluation.
//!
//! Reads one request per line from stdin until end of input and writes one response per
//! request line to stdout, in request order; see `advisor::matchup` for the protocol. The
//! only accepted argument is `--threads N`, how many requests run at once (default: every
//! hardware thread). Each solve is single-threaded, and results do not depend on N.

use std::env;
use std::io::{self, Write};
use std::num::NonZeroUsize;
use std::process;
use std::sync::Arc;

use urban_recreation_rust::advisor::matchup::{run, MatchupSources};
use urban_recreation_rust::advisor::search::default_search_threads;

fn threads(arguments: &[String]) -> Result<NonZeroUsize, String> {
    match arguments {
        [] => Ok(default_search_threads()),
        [flag, value] if flag == "--threads" => value
            .parse::<NonZeroUsize>()
            .map_err(|_| "--threads must be a positive integer".to_owned()),
        _ => Err("usage: urban-recreation-matchup [--threads N]".to_owned()),
    }
}

fn main() {
    let mut stderr = io::stderr();
    let arguments: Vec<String> = env::args().skip(1).collect();
    let threads = match threads(&arguments) {
        Ok(threads) => threads,
        Err(error) => {
            writeln!(stderr, "matchup: {error}").ok();
            process::exit(2);
        }
    };
    let sources = match MatchupSources::load() {
        Ok(sources) => Arc::new(sources),
        Err(error) => {
            writeln!(stderr, "matchup: {error}").ok();
            process::exit(2);
        }
    };
    let stdin = io::stdin();
    if let Err(error) = run(stdin.lock(), io::stdout(), sources, threads) {
        writeln!(stderr, "matchup: {error}").ok();
        process::exit(2);
    }
}
