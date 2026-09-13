/**
 * Verbose engine tracing: every condition tested, every modifier applied, every battle's
 * attack line. It is how the engine explains itself when a rule is checked by hand, and it
 * is ruinous in the search - the strings and their ANSI colours were built on every one of
 * millions of ability applications even with `console.log` stubbed to a no-op, measured at
 * 5.9% of solver runtime (see the performance notes in AGENTS.md).
 *
 * Guarded sites read `if (DEBUG) console.log(...)`. `DEBUG` is a module-level `const`, so
 * V8 treats the script-context slot as constant and TurboFan folds the branch away once the
 * code is hot: with tracing off, nothing is built and nothing is called.
 *
 * Off unless `UR_DEBUG=1`. `deno task run` sets it, so playing a game by hand narrates
 * itself exactly as it always has.
 */
function traceEnabled(): boolean {
  try {
    // Ask before reading. A bare `Deno.env.get` on a variable outside `--allow-env` makes
    // Deno *prompt* on an interactive terminal, and a permission prompt drawn over the
    // advisor's alt-screen is both unanswerable and corrupts the frame. querySync needs no
    // permission and never prompts, so tracing simply stays off where it was not granted.
    if (Deno.permissions.querySync({ name: "env", variable: "UR_DEBUG" }).state !== "granted") {
      return false;
    }
    return Deno.env.get("UR_DEBUG") === "1";
  } catch {
    return false;
  }
}

export const DEBUG: boolean = traceEnabled();
