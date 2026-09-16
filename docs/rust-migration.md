# Rust migration and parity plan

## Decision

The TypeScript and Rust implementations live in one repository. The Rust history was
imported without squashing under `rust/`, so its earlier development remains inspectable.

This is a shared-source monorepo, not two engines that must be edited in lockstep. New game
rules are established from captured server results and implemented in the TypeScript
reference first. Rust then consumes the same data and replay contract and is checked
independently against the same server results.

## Sources of truth

| Concern | Source of truth | Notes |
| --- | --- | --- |
| Card identity and level stats | `data/data.json` | One row per `(card id, level)`; do not use names as identity. |
| Effect definitions | `captures/abilities.json` | Versioned Rust compilation preserves the structured record and fails closed on unsupported semantics. |
| Observed game behavior | `captures/games/*.json` | Server power, damage, attack, winner, life, and pillz are the parity oracle. |
| Capture normalization | `scripts/ExtractBattle.ts` and the Rust replay adapter | Both must preserve side identity and the first-mover convention. |
| Working engine and advisor | TypeScript | Keep this stable while Rust is revived. |
| Candidate engine and solver backend | `rust/` | Must pass replay parity before advisor integration. |
| Old Rust assets and 10,000-case corpus | Historical baseline only | Useful for detecting accidental behavior changes, not evidence of current game correctness. |

The 53 TypeScript replay mismatches are known gaps in the reference, not expected Rust
answers. When TypeScript and a capture disagree, the capture wins after the evidence has
been checked.

## Non-negotiable gates

1. A structural Rust change must leave the archived historical semantic digest unchanged,
   unless the commit explicitly and intentionally changes game behavior.
2. Every Rust rule change must be tested against server-backed replay data. Prefer at least
   two independent captures for a newly inferred rule.
3. Rust replay failures must report the battle, round, cards, moves, and the exact field
   difference; aggregate pass counts alone are not enough.
4. The TypeScript replay baseline must be rerun after shared data or capture-contract
   changes.
5. Solver comparisons are valid only when both implementations use the same information
   model, legal moves, evaluator, and tie-breaking policy.

## Migration stages

### 1. Preserve and establish the library boundary

- Archive both pre-migration histories and non-ignored working trees.
- Import Rust under `rust/` with history.
- Make the Rust crate a normal library by default.
- Keep the old HTTP advisor behind the `legacy-advisor` feature.
- Remove unsafe diagnostic globals and scratch benchmarks without rule changes.
- Replace print-only tests with behavior tests and an ignored historical regression gate.

### 2. Share canonical inputs

- Index cards by `(id, level)` from `data/data.json`.
- Parse every `captures/games/*.json` file into a versioned Rust replay model.
- Normalize captured sides into engine-player order while retaining source-side identity.
- Validate the adapter against the embedded TypeScript testcase, without running either
  engine.

This stage proves that Rust can consume the live project's inputs. It does not claim engine
parity.

`EffectRegistryV1` is the effect-dictionary boundary. It validates every known structured
field and enum, detects key/id and description conflicts, and resolves captures strictly by
both id and description. Its string-free compiled values cover only reviewed unconditional
combat-stat modifiers, Support scaling, Stop Bonus, and combat-stat cancellation. All other
well-formed definitions—including textual Team/Day/Night context not represented by
`abilityData`—remain explicit `Unsupported`; classified does not mean executed or parity-tested.

The registry itself is replay-model neutral: replay preparation owns absent/present source
mapping and calls strict id-and-description lookup rather than the registry importing capture
types.

### 3. Reach engine parity vertically

- Introduce replay execution without coupling the adapter to the historical `Game` type.
- Start with legal selection, turn order, base attack, round winner, damage, resources, and
  game status.
- Add clan bonuses, conditions, cancels/protection, post-round effects, and permanent
  effects in evidence-backed slices.
- Report Rust-vs-server and TypeScript-vs-server results separately.
- Only remove historical Rust data structures when their replacement is covered by the
  replay suite.

The target is exact agreement for every replay-ready capture: per-round power, damage,
attack, winner, life, and pillz. Incomplete and Dojo captures remain classified rather than
silently discarded.

In the 328-capture corpus at import time, only 58 of 1,102 asserted rounds reduce to base
power, damage, attack, cost, and tie rules when effects are disabled. Only 20 of those are
reachable as uninterrupted prefixes, and battle `1065231` is the sole meaningful complete
capture in that subset. Use those fixtures to prove replay plumbing, not ability support:
some printed effects cancel each other and happen to leave a base-stat result.

The current captures fit eight-bit combat values, but that is not a durable type contract.
Replay and new engine boundaries should use wider integer types so future modifiers and
solver-generated states cannot silently overflow.

The first current-engine slice now lives in `rust/src/engine/`, alongside rather than inside
the frozen historical engine. `BaseRulesMatchSpec` is immutable match context (including
numeric clan IDs, night, and battle-rule identity), while `BaseRulesPosition` is the small,
structurally comparable mutable state. `BaseRulesGame::make` validates an entire round before
mutation and returns an opaque snapshot undo for exact `unmake`.

`BaseRulesReplay` revalidates the public replay model, binds cards to the canonical catalog,
and executes capture selections without consulting expected outputs. Its execution APIs and
reports are explicitly named effects-disabled/base-rules: they currently establish only
attack, Fury, tie, resource, life, and status plumbing. The fixed server-backed boundary is
20 uninterrupted rounds across 18 captures, including complete two-round battle `1065231`;
it is not a claim that printed abilities or bonuses are implemented.

The next vertical slice is deliberately named `ClanBonusDiagnostic`, not a full-effects
engine. The replay-preparation constructor `ClanBonusDiagnosticReplayV1::new` requires the
explicit policy that disables ordinary abilities and out-of-slice bonuses, then constructs
the string-free `ClanBonusDiagnostic` plan. Every captured ability and bonus remains visible
as Execute, Disabled, or Absent in preparation metadata, selected-round reports, and failure
context. Execute means admitted by this projection, not guaranteed to apply: Stop Bonus or
combat-stat cancellation may suppress an admitted effect. Exact registry conflicts are fatal,
while unsupported variants of the Stop Bonus/stat-cancellation controls promised by the
projection fail atomically only if their card is selected. Preparation and reports retain the
policy, registry schema version, and the registry's non-cryptographic FNV-1a source-byte
fingerprint so results identify the exact compilation boundary.

This diagnostic trusts the capture's active `source_bonus`: null is inactive, while a
present exact id/description can already reflect Oculus infiltration, Day/Night, or an
Oblivion copy. Support is replay-specific source-bonus context: the count of distinct card
ids in the immutable whole draw sharing that active source-bonus id, including played cards.
It is not inferred clan membership and must not be reused as the future catalog-only solver
constructor. The fixed gate preserves all 20 base prefixes and adds 20 more, for 40 exact
server-backed rounds; `876712` is the one newly complete capture. Night bonus id `1442`
remains explicitly Unsupported and deferred.
Several prefixes are exact because paired omitted effects are both deliberately Disabled;
the cancellation, directional-clamp, and control-cycle edge semantics are pinned primarily
by focused synthetic tests rather than independently observable outcomes throughout this gate.
Capture `1089974` round index 2 independently establishes source ownership for cancellation:
Dookor's cancel suppresses Sue's Power-and-Damage reduction while Dookor's own opponent-Power
reduction still applies (Sue 6 to 4). The focused resolver test isolates that ordering without
claiming that this projection executes ordinary numeric abilities.

`ClanBonusDiagnostic::match_spec()` exposes the complete immutable compact plan, including
every source disposition identity and Support count. A future transposition key must combine
the diagnostic model, this full match identity, `position()`, and the caller's explicit next
first mover, which is not derivable from round parity. The partial base-rules view is
deliberately named `base_rules_spec()`. This remains replay-prepared diagnostics only: it
neither infers active bonuses from catalog clans nor enables conditions, ordinary numeric
abilities, post-round effects, permanents, protection, or out-of-slice bonuses.

`CombatStatDiagnosticV1` is the next separate replay-prepared projection; it does not widen
`ClanBonusDiagnostic` or claim full engine parity. It executes reviewed fixed ordinary
Power, Damage, Power-and-Damage, and Attack abilities alongside the existing fixed and
Support bonuses, Stop Bonus, and source-owned combat-stat cancellation. The only admitted
numeric predicates are `Always`, Courage (`OwnerMovesFirst`), Reprisal
(`OwnerMovesSecond`), Symmetry (`SelectedHandSlotsMatch`), and Asymmetry
(`SelectedHandSlotsDiffer`). Courage and Reprisal use the round's explicit first mover;
Symmetry and Asymmetry compare the two immutable original hand slots, not card identity or
current stats. The index predicates are admitted for fixed numeric abilities and bonuses.
Positional and index effects require otherwise-neutral structured fields and an exact
description body matching their typed stat, magnitude, and bound; an unfamiliar nested
context fails closed. Conditional Stop Bonus, cancellation, copy, and protection remain
outside this slice even when their predicate would be false.

Resolution retains Bonus-then-Ability source compilation for own increases. Opponent
Power/Damage reductions and opponent Attack reductions are independently stable-sorted by
descending minimum, with Bonus before Ability on an equal minimum. This reproduces the
server evidence from Robb/All Stars (`1011768`, 6 to 4 to 2), Don Cr/Montana
(`875272`/`901613`, attack 18 to 8 to 4), and Miss Stella/Sakrohm (`901292`, attack 18 to 11
to 3). Fury follows Power/Damage resolution; base Attack follows Fury; own Attack increases
then precede the sorted opponent Attack reductions. Arithmetic observations outside an
admitted sequential prefix remain focused evidence rather than replay-gate members.

The immutable server-backed gate is thirteen sequential prefix rounds:
`875032/1`, `875155/1`, `1088323/1`, `1081463/1`, `1089513/1`, `901400/1`, and
`874837/2`, plus `1011643/2`, `1011768/1`, and `1011483/2`. Its selected
Execute/Disabled identity sets are pinned, while focused tests pin the new predicate
assignments and branches. `1011483` visibly proves active Asymmetry (Galahad Damage 2 to 5 on unequal slots) and
active Symmetry (Anagone reduces Bella Ld Power 7 to 4 on equal slots); `1011768` visibly
proves inactive Asymmetry (Aneta remains Damage 3 on equal slots); and `1011643` proves an
active Asymmetry bonus is still suppressed by Stop Bonus. Additional arithmetic evidence for
both branches comes from Olivia (`1092515` round 2 / `1092660` round 1), Fiend (`963694`
round 1 / `945724` round 1), K Cube (`878056` round 3 / `875322` round 2), and Anagone
(`1011016` round 1 / `1010898` round 3), using zero-based capture round numbers. `877812`
continues to reject selected Degrowth in
round zero. The gate now contains observable active Courage and Reprisal cases; their inactive
branches and the complete hand-slot predicate matrix are also pinned synthetically. Replay
provenance records compiler/policy semantic revision 2 for this scope.

Replay preparation scans all eight cards. Canonical Leader clan id 36 and Team/global or
Mock/Illusion sources are fatal even when unplayed, because they may execute off-card.
Unsupported card-local controls and every unadmitted current-round combat-stat modifier are
retained as visible Disabled metadata but reject atomically if selected. Ordinary Support
abilities never reuse the source-bonus Support count; capped increases also remain deferred
for lack of clean evidence. Life, pillz, post-round, and permanent effects are explicitly
disabled by the projection. Provenance records the model, explicit projection policy,
registry schema and non-cryptographic source fingerprint, plus a combined model-specific
compiler/policy semantic revision. A transposition identity must include that full match
specification, the model, `position()`, and the explicit next first mover.

### 4. Port current solver semantics

Do not revive the old perfect-information recommendation model as the live advisor. Port
the current TypeScript behavior deliberately:

- allocation-free make/unmake search;
- depth-2 work units and cancellation;
- the conservative information-aware policy for hidden pillz and Fury;
- opening heuristic and captured-move weighting;
- blind-second handling;
- visible-percent, knockout, safety, then cost ranking.

Keep the old Rust solver available as a historical reference until equivalence tests cover
the intended replacement.

### 5. Integrate Rust behind a process boundary

Use a versioned JSON-lines worker protocol initially. The TypeScript advisor remains the
owner of capture state, policy selection, cancellation, and terminal rendering. A worker
process gives clean crash isolation and makes A/B comparison straightforward. In-process
FFI is only worth considering after the protocol and engine are stable.

## Performance measurement

Measure engine and solver performance separately:

- Engine: replay identical normalized rounds and report time per resolved round.
- Search: solve identical positions with identical semantics and report nodes, elapsed
  time, peak memory, and result checksum.
- Build Rust with `--release`; warm both runtimes; use multiple alternating samples; keep
  debug output disabled; record machine and commit hashes.

A faster answer from a different policy or a smaller tree is not an implementation speedup.
Correctness and semantic equivalence are gates before headline comparisons.

## Working commands

```bash
# TypeScript reference
deno test -A --no-check tests/replay/
deno test -A --no-check

# Rust foundation
cargo fmt --manifest-path rust/Cargo.toml -- --check
cargo check --manifest-path rust/Cargo.toml --locked --all-features
cargo test --manifest-path rust/Cargo.toml --locked
```
