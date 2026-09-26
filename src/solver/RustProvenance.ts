// The Rust V1 data provenance, read from the checkout: byte-for-byte fingerprints of the
// catalog and effect registry the Rust side loads, and the semantic revisions its sources
// declare. Kept apart from RustAdvisorInput so tooling that only needs these numbers (the deck
// matchup CLI) does not load the TypeScript engine with them.
import type { RustProvenance } from "./RustAdvisor.ts";

/** This must remain the V1 value in rust/src/effect_registry.rs. */
export const EFFECT_REGISTRY_SCHEMA_VERSION_V1 = 1;

const encoder = new TextEncoder();
const FNV_OFFSET = 0xcbf29ce484222325n;
const FNV_PRIME = 0x100000001b3n;
const U64_MASK = 0xffffffffffffffffn;

function fnv1a64(bytes: Uint8Array): string {
  let hash = FNV_OFFSET;
  for (const byte of bytes) {
    hash = ((hash ^ BigInt(byte)) * FNV_PRIME) & U64_MASK;
  }
  return hash.toString(16).padStart(16, "0");
}

function u64le(value: number): Uint8Array {
  const out = new Uint8Array(8);
  new DataView(out.buffer).setBigUint64(0, BigInt(value), true);
  return out;
}

function joinBytes(parts: readonly Uint8Array[]): Uint8Array {
  const out = new Uint8Array(
    parts.reduce((total, part) => total + part.length, 0),
  );
  let offset = 0;
  for (const part of parts) {
    out.set(part, offset);
    offset += part.length;
  }
  return out;
}

/**
 * Read and fingerprint the exact raw worker inputs.  In particular, do not parse and
 * re-serialise JSON: whitespace changes are deliberately part of Rust's provenance.
 */
export async function readRustV1Provenance(): Promise<RustProvenance> {
  const root = new URL("../../", import.meta.url);
  const [
    registry,
    catalog,
    overrides,
    registrySource,
    compilerSource,
    catalogMatchSource,
    advisorSearchSource,
  ] = await Promise.all([
    Deno.readFile(new URL("captures/abilities.json", root)),
    Deno.readFile(new URL("data/data.json", root)),
    Deno.readFile(new URL("data/battle_card_overrides.json", root)),
    Deno.readTextFile(new URL("rust/src/effect_registry.rs", root)),
    Deno.readTextFile(
      new URL("rust/src/engine/combat_stat_compiler.rs", root),
    ),
    Deno.readTextFile(new URL("rust/src/engine/catalog_match.rs", root)),
    Deno.readTextFile(new URL("rust/src/advisor/search.rs", root)),
  ]);
  const version = /pub const EFFECT_REGISTRY_SCHEMA_VERSION:\s*u16\s*=\s*(\d+);/
    .exec(registrySource)?.[1];
  if (
    version === undefined ||
    Number(version) !== EFFECT_REGISTRY_SCHEMA_VERSION_V1
  ) {
    throw new Error(
      "Rust V1 effect-registry schema version is missing or unsupported",
    );
  }
  const semanticRevision = (
    source: string,
    name: string,
  ): number => {
    const revision = new RegExp(
      `(?:pub(?:\\(crate\\))?\\s+)?const\\s+${name}:\\s*u16\\s*=\\s*(\\d+);`,
    ).exec(source)?.[1];
    if (revision === undefined) {
      throw new Error(`Rust semantic revision ${name} is missing`);
    }
    return Number(revision);
  };
  const effective = joinBytes([
    encoder.encode("urban-recreation-effective-catalog-v1\0"),
    u64le(catalog.length),
    catalog,
    u64le(overrides.length),
    overrides,
  ]);
  return {
    effectiveCatalogFingerprintFnv1a64: fnv1a64(effective),
    effectRegistryFingerprintFnv1a64: fnv1a64(registry),
    effectRegistrySchemaVersion: Number(version),
    compilerPolicySemanticRevision: semanticRevision(
      compilerSource,
      "COMBAT_STAT_COMPILER_POLICY_SEMANTIC_REVISION_V1",
    ),
    catalogContextPolicySemanticRevision: semanticRevision(
      catalogMatchSource,
      "CATALOG_CONTEXT_POLICY_SEMANTIC_REVISION_V1",
    ),
    advisorPolicySemanticRevision: semanticRevision(
      advisorSearchSource,
      "ADVISOR_POLICY_SEMANTIC_REVISION_V1",
    ),
  };
}
