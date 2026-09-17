// Build then execute the optional release-worker integration gate. Keeping this out of
// ordinary `deno test` makes the broad TypeScript suite independent of Cargo availability.
const cargo = await new Deno.Command("cargo", {
  args: [
    "build",
    "--manifest-path",
    "rust/Cargo.toml",
    "--release",
    "--locked",
    "--bin",
    "urban-recreation-advisor-jsonl",
  ],
  stdout: "inherit",
  stderr: "inherit",
}).output();
if (!cargo.success) Deno.exit(cargo.code);

const test = await new Deno.Command(Deno.execPath(), {
  args: [
    "test",
    "-A",
    "--no-check",
    "tests/solver/RustWorker.integration.test.ts",
  ],
  stdout: "inherit",
  stderr: "inherit",
}).output();
Deno.exit(test.code);
