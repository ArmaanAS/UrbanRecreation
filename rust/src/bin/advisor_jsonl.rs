//! One-request JSONL boundary for the TypeScript advisor host.
//!
//! The only accepted argument is `--threads N`, which fixes the root search's worker count
//! (default: every hardware thread). It is a process setting, not part of the protocol.

use std::env;
use std::io::{self, Write};
use std::num::NonZeroUsize;
use std::process;

use urban_recreation_rust::advisor::search::default_search_threads;

fn threads(arguments: &[String]) -> Result<NonZeroUsize, String> {
    match arguments {
        [] => Ok(default_search_threads()),
        [flag, value] if flag == "--threads" => value
            .parse::<NonZeroUsize>()
            .map_err(|_| "--threads must be a positive integer".to_owned()),
        _ => Err("usage: urban-recreation-advisor-jsonl [--threads N]".to_owned()),
    }
}

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stderr = io::stderr();
    let arguments: Vec<String> = env::args().skip(1).collect();
    let threads = match threads(&arguments) {
        Ok(threads) => threads,
        Err(error) => {
            writeln!(stderr, "advisor-jsonl: {error}").ok();
            process::exit(2);
        }
    };
    if let Err(error) = urban_recreation_rust::advisor::jsonl::run_with_threads(
        stdin.lock(),
        stdout.lock(),
        &mut stderr,
        threads,
    ) {
        writeln!(stderr, "advisor-jsonl: {error}").ok();
        process::exit(2);
    }
}
