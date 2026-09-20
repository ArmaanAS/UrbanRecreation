/**
 * Regenerable expectation files, the TypeScript half of `rust/tests/support/mod.rs`.
 *
 * Provenance fingerprints move whenever the catalog, the captured ability registry or a
 * Rust semantic revision changes - which is every landed slice and every capture batch.
 * Hand-copying sixteen hex digits is the kind of bookkeeping nobody can check by eye, so
 * the expected value lives in `tests/expect/<name>.json` and is regenerated:
 *
 * ```bash
 * UR_UPDATE_EXPECT=1 deno test -A --no-check tests/solver/RustAdvisorInput.test.ts
 * git diff tests/expect   # review before committing
 * ```
 *
 * Without the variable a moved value fails the test, printing both sides.
 */

import { assertEquals } from "@std/assert";

const UPDATE_ENV = "UR_UPDATE_EXPECT";

function updating(): boolean {
  const value = Deno.env.get(UPDATE_ENV);
  return value !== undefined && value !== "" && value !== "0";
}

function expectUrl(name: string): URL {
  return new URL(`../expect/${name}.json`, import.meta.url);
}

interface Expectation {
  description: string;
  regenerate: string;
  value: unknown;
}

/**
 * Compare `actual` against the stored expectation, or rewrite it under `UR_UPDATE_EXPECT`.
 */
export async function expectSnapshot(
  name: string,
  description: string,
  actual: unknown,
): Promise<void> {
  const url = expectUrl(name);
  const write = async () => {
    const body: Expectation = {
      description,
      regenerate:
        `UR_UPDATE_EXPECT=1 deno test -A --no-check tests/solver/RustAdvisorInput.test.ts`,
      value: actual,
    };
    await Deno.writeTextFile(url, `${JSON.stringify(body, null, 2)}\n`);
  };

  let stored: Expectation | undefined;
  try {
    stored = JSON.parse(await Deno.readTextFile(url)) as Expectation;
  } catch (error) {
    if (!(error instanceof Deno.errors.NotFound)) throw error;
    if (!updating()) {
      throw new Error(
        `expectation file ${url.pathname} is missing; regenerate with ${UPDATE_ENV}=1`,
      );
    }
    await write();
    return;
  }

  if (JSON.stringify(stored.value) === JSON.stringify(actual)) return;
  if (updating()) {
    console.error(
      `${name}: ${JSON.stringify(stored.value)} -> ${JSON.stringify(actual)}`,
    );
    await write();
    return;
  }
  assertEquals(
    actual,
    stored.value,
    `${name} moved; regenerate with ${UPDATE_ENV}=1 and review the diff before committing`,
  );
}
