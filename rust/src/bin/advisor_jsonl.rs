//! One-request JSONL boundary for the TypeScript advisor host.

use std::io::{self, Write};
use std::process;

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stderr = io::stderr();
    if let Err(error) =
        urban_recreation_rust::advisor::jsonl::run(stdin.lock(), stdout.lock(), &mut stderr)
    {
        writeln!(stderr, "advisor-jsonl: {error}").ok();
        process::exit(2);
    }
}
