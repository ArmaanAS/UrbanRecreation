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
| Card identity and level stats | `data/data.json` plus `data/battle_card_overrides.json` | One row per `(card id, level)`; solver-facing Rust construction validates and applies the reviewed runtime overrides used by TypeScript. Do not use names as identity. |
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

`CardCatalog` also validates a 36-entry clan index derived from the card rows. The distinct
`EffectiveCardCatalog` type requires an explicit override contract and accepts each override
only when a row equals either its reviewed `from` definition or its already-updated `to`
definition. Any missing card, duplicate override, name mismatch, or unexpected third state
fails construction. This currently corrects Quetzal Cr `(1577, 3)` from the stale raw 2/6
No Ability row to the captured 7/4 Stop Opp. Bonus definition and live ability id 5927.
The effective catalog fingerprints the exact catalog and override bytes it validated.

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
structurally comparable mutable state. Its owner-relative `previous_round_winner` is `None`
before round zero and is updated only after a resolved round, so it also round-trips through
the opaque snapshot undo. `BaseRulesGame::make` validates an entire round before mutation and
returns that undo for exact `unmake`.

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

`CombatStatDiagnosticV1` is the next separate projection, first exercised through replay
preparation and now also materialized by the strict catalog constructor described below. It
does not widen `ClanBonusDiagnostic` or claim full engine parity. It executes reviewed fixed ordinary
Power, Damage, Power-and-Damage, and Attack abilities alongside the existing fixed and
Support bonuses, ordinary unconditional Support Attack/Power/Damage abilities, Stop Bonus,
and source-owned combat-stat cancellation. The only admitted
numeric predicates are `Always`, Courage (`OwnerMovesFirst`), Reprisal
(`OwnerMovesSecond`), Symmetry (`SelectedHandSlotsMatch`), Asymmetry
(`SelectedHandSlotsDiffer`), Confidence (`OwnerWonPreviousRound`), and Revenge
(`OwnerLostPreviousRound`). Courage and Reprisal use the round's explicit first mover;
Confidence and Revenge compare the selected owner's identity with the resolved winner of the
immediately preceding round, so both are false in round zero; Symmetry and Asymmetry compare
the two immutable original hand slots, not card identity or current stats. The index predicates
are admitted for fixed numeric abilities and bonuses. Positional, index, and bounded
previous-round effects require otherwise-neutral structured fields and an exact description
body matching their typed stat, magnitude, and bound; an unfamiliar nested context fails
closed. Conditional Stop Bonus, cancellation, copy, and protection remain outside this slice
even when their predicate would be false.

Growth and Degrowth are magnitude multipliers rather than predicates. From the immutable
pre-commit zero-based `rounds_played`, Growth uses factors `1, 2, 3, 4` and Degrowth uses
`4, 3, 2, 1`; the complete multiplied change is then clamped once. Replay preparation maps
only exact `isOverdrive`/`isDivide` structured shapes with an exact `Growth: ` or
`Degrowth: ` numeric body. Round-scaled life, pillz, post-round/permanent effects,
own-stat decreases, nested conditions, and a round multiplier combined with another
predicate remain fail-closed. Both abilities and bonuses are admitted because Dominion's
captured bonus id 1578 is genuinely Growth. The older clan-only diagnostic explicitly
rejects these new shared resolver magnitudes at its public plan boundary.

Equalizer is likewise a magnitude rather than a predicate. The admitted combat-stat subset
multiplies its complete change by the selected opponent card's exact catalog level (its star
count), then applies the usual bound once. Compilation requires an otherwise-neutral
`isOppStarsLinked` structure and an exact `Equalizer: ` numeric body. Ability- and
bonus-origin Equalizer share the same selected-opponent context; Equalizer life and pillz,
nested conditions, and every other linked magnitude remain fail-closed. The catalog lookup
and compilation happen once during match construction, while round resolution reads only the
already-selected opponent `CardKey.level`. The older clan-only diagnostic rejects this new
shared resolver magnitude at its public plan boundary.

Resolution retains Bonus-then-Ability source compilation for own increases. Opponent
Power/Damage reductions and opponent Attack reductions are independently stable-sorted by
descending minimum, with Bonus before Ability on an equal minimum. This reproduces the
server evidence from Robb/All Stars (`1011768`, 6 to 4 to 2), Don Cr/Montana
(`875272`/`901613`, attack 18 to 8 to 4), and Miss Stella/Sakrohm (`901292`, attack 18 to 11
to 3). Fury follows Power/Damage resolution; base Attack follows Fury; own Attack increases
then precede the sorted opponent Attack reductions. Arithmetic observations outside an
admitted sequential prefix remain focused evidence rather than replay-gate members.

The immutable server-backed gate is forty-four sequential prefix rounds:
`875032/2`, `875155/1`, `1088323/1`, `1081463/1`, `1089513/2`, `901400/2`, and
`874837/2`, plus `1011643/2`, `1011768/1`, `1011483/2`, `877812/2`, and
`874642/1`, `1059269/1`, and `1091585/1`, plus `868094/1`, `875230/1`, and
`877950/1`, plus `945585/2`, `1023396/2`, `874962/2`, and `946400/1`, plus
`1058366/3` and `1061897/4`, plus `946288/1`, `1092660/1`, and `1093500/2`, plus
`1092909/2`. Its selected Execute/Disabled/Absent identity sets are pinned, while focused
tests pin the new predicate assignments and branches. `1011483` visibly proves active Asymmetry
(Galahad Damage 2 to 5 on unequal slots) and active Symmetry (Anagone reduces Bella Ld Power
7 to 4 on equal slots); `1011768` visibly proves inactive Asymmetry (Aneta remains Damage 3
on equal slots); and `1011643` proves an active Asymmetry bonus is still suppressed by Stop
Bonus. Additional arithmetic evidence for both branches comes from Olivia (`1092515` round 2
/ `1092660` round 1), Fiend (`963694`
round 1 / `945724` round 1), K Cube (`878056` round 3 / `875322` round 2), and Anagone
(`1011016` round 1 / `1010898` round 3), using zero-based capture round numbers.

The gate extends `1089513` to `/2` and adds `877812/2` plus `874642/1` for round-scaled
evidence. `1089513` round 1 proves Growth Attack +3 uses factor 2 (base attack 10 becomes
16); `874642` round 0 proves Degrowth Power-and-Damage +1 uses factor 4 before Lothar's
Power reduction (Nidory reaches 10/5, then 7/5); and `877812` supplies a Degrowth
observation consistent with a numerically clamped no-op when its target is already at the
minimum. Capture `878056` separately pins cancellation suppressing active Degrowth. The gate
contains observable active Courage and Reprisal cases; their inactive branches, the complete
hand-slot predicate matrix, and all four round factors are also pinned synthetically.
Capture `1059269` proves the Hive Equalizer bonus scales from Callie's three stars and clamps
her Attack to 5 after Stop Bonus suppresses Rescue. Capture `1091585` exercises both Aegis
Cr's Equalizer Power ability and the Hive Equalizer Attack bonus against three-star Nidory,
in the same round as Growth and Degrowth. Focused tests cover opponent levels one through
five and exact make/unmake restoration. Oscar (`868094`) proves Support Power +1 with four
matching characters before Callie's opposing reduction; Ludicrite (`875230`) and Boohma
(`877950`) prove Support Attack +3 and +5 respectively with four matching characters.
Focused catalog tests also pin singleton Taljion and Oculus-derived effective-clan counts.
The Riots prefixes prove exact post-round Pillz accounting on both outcomes. In `1058366`,
Astromos receives the bonus after winning round 0, the non-Riots Rhum'n'Bass receives
nothing in round 1, and Kenjy Noel receives it after losing by KO in round 2. In `1061897`,
all four selected Riots cards receive the bonus, with the resulting resource sequence pinned
against the capture. That KO observation exposed a narrow TypeScript guard error: the
reference engine now permits this exact bonus Pillz gain after the owner reaches zero life,
fixing eight captured replays while lethal Kubra compound-Defeat controls remain suppressed.
Replay provenance records compiler/policy semantic revision 12 for the current scope.

The four revision-10 gate rounds exercise the exact same-text ability family without
turning description equality into authority. Bonnie Ld level 2 (`946288`, registry `5085`)
and level 1 (`1092660`, registry `5520`) receive one Pillz after losing; Bonnie's Stop Bonus
also suppresses Agnes's Riots bonus in the latter round. `1093500` first executes a captured,
dynamically copied `ability:1034`, then executes Pr Hide's printed `ability:1375` together
with her Riots bonus. Capture `1092909` round 3 separately proves Pr Hide receives both gains
after a KO; a focused make/unmake test pins the same +2 transition because the preceding
Argos round remained outside revision 10's gate.

Revision 11 extends `1092909` through round 1. Round 0 re-exercises an admitted Riots bonus;
round 1 then pins Argos' exact `ability:1158` after costs and the Riots bonus: 9 carried
Pillz minus 2 paid becomes 7, the bonus makes 8, and Argos makes 10 after losing. This adds
two sequential rounds and raises the immutable gate from 42 to 44.

The carried-forward previous-round portion admits only the exact fixed numeric grammar: a structured
`previousRoundRequirement` of `win` with `Confidence: ` or `Confidence : `, or `lose` with
`Revenge: `; the suffix must satisfy the existing exact fixed combat-stat grammar. The effect
must otherwise be neutral (`currentRoundRequirement=any`, no other condition, link,
inversion, permanence, or special action) and remain a player increase or opponent decrease
of Attack, Power, Damage, or Power-and-Damage. This applies to ordinary abilities and to the
observed Frozn Revenge bonus id `801`; its normal bonus liveness and Stop Opp. Bonus
cancellation ordering still apply. The added replay prefixes prove active Confidence
(`875032/2`), active Revenge reduction (`945585/2`), the inactive then active id-801 bonus
transition (`1023396/2`), and active Revenge Power-and-Damage (`874962/2`). They do not
establish a general prior-round-condition model.

The remaining 21 observed prior-round shapes remain explicit deferred/disabled records:
Stop Opp. Ability (`490`, `589`, `1680`), copy/exchange (`1409`, `1713`, `1751`, `4972`),
Night-prefixed Confidence (`1643`), dynamic conversion (`1719`), current-round
conjunctions and life/pillz effects (`814`, `1652`, `1661`, `1702`, `1810`, `2113`, `3016`,
`3546`, `4301`, `4449`), and permanent Mindwipe/Poison (`2582`, `3301`).

Semantic revision 8 adds one deliberately narrow post-round resource effect: the exact
structured and textual `Defeat: Recover 2 Pillz Out Of 3` shape, with an unconditional
`previousRoundRequirement=any`, current-round `lose`, player Pillz increase, `recover_pillz` special
action, values `2` and `3`, and every unrelated link, bound, condition, permanence, and
multiplier neutral. It is identity- and source-kind-locked: registry id `577` only as a
bonus, and ids `729` and `1418` only as abilities. Same-text id `2475`, every other source
kind pairing, and every other recovery ratio or shape remain Disabled/fail-closed; this is
not generic Defeat, Pillz, or post-round-effect parity. `901400/2` proves ordinary
bonus-577 recovery, while `946400/1` proves ability-1418 recovery. Ability 729 is admitted
by the same strict compiler and a synthetic Fury-cost test, but capture `1024592/2` remains
preparation-only: Arnie's 729 is `ExecutePostRound` while unrelated selected id `1090` is
Disabled, so the prefix intentionally mismatches rather than widening that effect.

Recovery uses the losing card's paid cost after payment: the free attack pill is excluded,
Fury's three paid Pillz are included, and it restores `max(1, ceil(2 * paid / 3))`. It is
applied after winner/damage resolution to the loser, including a KO, before the committed
round's status/report and final position are exposed. It is a typed post-round plan, not a
combat-stat modifier; the base commit builds a complete replacement position, so recovery
overflow aborts atomically and the opaque undo restores the pre-round position exactly.
Existing Stop Opp. Bonus liveness suppresses bonus-577 recovery (`945585`); Stop Opp.
Ability, protection, and Pillz-cancellation interactions are still unsupported and
fail-closed if selected.

Semantic revision 9 introduced one further exact post-round resource effect: Riots'
`Victory Or Defeat : +1 Pillz`, registry definition `1034` as a bonus. Revision 10 extends
that already-typed effect to the audited source/identity pairs `ability:1034` and printed
abilities `1375`, `4111`, `5085`, and `5520`. Its structured
shape must be otherwise neutral and unconditional, target the owning player's Pillz, use a
fixed increase of one, and carry no special action, bounds, links, flags, permanence,
Support, or multiplier. Every other identity, source-kind pairing, description, or shape
remains Disabled and rejects if selected. This is not general Victory-or-Defeat,
Pillz-increase, Copy, or same-text ability parity.

The evidence is intentionally recorded with its limits. Pr Hide `1375`, Bonnie Ld `5085`,
and Bonnie Ld `5520` each have multiple selected observations; the copied replay identity
`ability:1034` has four. Alba `4111` has one selected observation, but its exact registry id,
description, and complete structured payload are identical to the observed family. Direct
post-KO ability evidence currently comes from Pr Hide; applying the same post-KO semantics to
the other exact identities is a bounded inference from that shared payload, not a claim about
unlisted same-text cards such as Atess.

The effect adds exactly one Pillz after move costs and damage resolution for either outcome,
including when its owner is KO'd. Bonus executes before ability when a card owns both,
matching the TypeScript phase-registration order; normal Stop Bonus liveness suppresses only
the bonus. Post-round execution therefore visits both
selected owners for this typed plan, while recovery remains loser-only. Checked addition
reports a distinct `PillzIncreaseOverflow` and aborts before commit, and the opaque undo
restores the exact pre-round position and hash.

Semantic revision 11 admits one capped increase without widening the generic boundary:
Argos' exact `ability:1158`, `Defeat: +2 Pillz Max. 11`. The complete structured record must
be a fixed increase of two to the owning player's Pillz, capped at 11, with
`currentRoundRequirement=lose`; every other condition, link, flag, permanence field and
special action must be neutral. The source kind, registry id, printed description and full
shape are all authority. Catalog construction maps only Argos level 2's printed ability id
1158 to registry definition 1158; Argos level 1 has no ability, and description equality or
dynamic Copy cannot synthesize the effect.

Execution follows the TypeScript END phase after costs and damage. A live clan bonus runs
before the ability. If Argos loses and survives, a value below 11 gains two and clamps at
11; a value already at or above 11 is left unchanged rather than pulled down. Captures cover
post-cost values 0, 2, 4, 7, 8 and 11, a win, Stop Ability, and combined Stop Ability/Stop
Bonus. In particular, `1093451` proves 11 becomes 12 from Riots before Argos observes that
the value is already capped. Stop Bonus suppresses only Riots; Argos remains an ability.
The only captured Argos KO also has Stop Ability active, so it does not establish live-server
post-KO behavior. The Rust slice deliberately follows the current TypeScript reference and
suppresses Argos at zero life; this is recorded as a bounded inference pending an active
post-KO capture. Focused tests pin bonus-before-ability ordering, zero residual Pillz, wins,
the cap, above-cap preservation, KO suppression, Stop Bonus isolation, and exact undo/hash
restoration.

Semantic revision 12 adds a generic-but-narrow Victory Life plan.  A source is admitted only
when its description is exactly `+{N} Life` for its positive structured `value`, targets the
owning player's Life with `increase`, has `currentRoundRequirement=win`, and is otherwise
neutral: no bounds, links, Support flags, condition, special action, multiplier, or
permanence.  Both Ability and Bonus sources may use that grammar.  The typed plan runs after
costs and damage, Bonus before Ability, for the round winner only; checked Life addition is
atomic and status is calculated afterward. Capture `877636` is the complete four-round gate:
Dave's normalized round 2 win turns the post-damage 12 Life into 14.  This is not generic
Life-effect parity.

Catalog construction does not treat description equality as authority for this generic rule.
Its numeric catalog id must be the resolved registry definition id or a structurally identical
registry alias, except for the explicit active-Jungo bridge from catalog bonus `41` to captured
registry definition `401`. That bridge is clan-, source-kind-, id-, description-, and
structure-gated; captures `877860/1` and `878011/1` independently pin winning Jungo bonus
arithmetic. Metadata retains the catalog id, resolved definition id, and complete alias set.
A mismatch rejects the strict draw. Compiler/policy provenance is revision 12.

Replay preparation scans all eight cards. Canonical Leader clan id 36 and Team/global or
Mock/Illusion sources are fatal even when unplayed, because they may execute off-card.
Unsupported card-local controls and every unadmitted current-round combat-stat modifier are
retained as visible Disabled metadata but reject atomically if selected. The 33 observed
ordinary Support combat-stat definitions are admitted only for an otherwise-neutral,
unconditional basic Attack, Power, or Damage shape. Conditional, nested, life, pillz,
Power-and-Damage, post-round, and permanent Support effects remain fail-closed; every capped
increase except exact `ability:1158` remains deferred. Only the five exact audited
Victory-or-Defeat ability identities are admitted; all other same-text sources remain
rejected when selected, so this family does not widen the generic resource-effect boundary.
Provenance records the model,
explicit projection policy,
registry schema and non-cryptographic source fingerprint, plus a combined model-specific
compiler/policy semantic revision. A transposition identity must include that full match
specification, the model, `position()`, and the explicit next first mover.

The combat-stat plan validates Support context by effective clan rather than captured effect
id: it counts distinct character ids in the immutable draw that share the source card's
effective clan. Executable ability Support and active bonus Support carry independently
validated counts, so either source can be absent or stopped without borrowing the other's
context. Compiler/policy revision 12 records this semantic boundary together with the exact
Equalizer multiplier, the bounded Confidence/Revenge/Frozn slice, the identity-locked
Defeat-recovery post-round plan, the identity-locked Victory-or-Defeat Pillz family with its
both-owner post-round execution semantics, Argos' identity-locked capped Defeat gain, and the
exact-structured positive Victory Life plan.

`CatalogCombatStatMatchV1` is the first strict, replay-independent constructor intended for
future solver work. Its input contains battle-rule id, explicit day/night state, initial
life and pillz, and exactly four `CardKey`s per player. It materializes base stats plus the
complete compact combat plan once; catalog and registry lookups never enter round execution
or the eventual search hot path. Unlike replay preparation, it rejects the whole draw when
any of the eight cards has an unsupported active source, because a solver must be total over
every legal selection rather than defer failure until a card is played.

Effective clan derivation is pure match context. It retains canonical clan separately,
counts distinct character ids across the immutable original draw, and never mutates catalog
rows. With exactly one Oculus, `O+A+A+A` infiltrates A, `O+A+A+B` infiltrates singleton B,
and `O+A+B+C` does not infiltrate; multiple Oculus cards disable infiltration. An infiltrated
card receives the target clan's selected day/night printed bonus before activation and
Support counting. Duplicate character ids and any Leader are rejected by strict solver
construction. Catalog source ids remain distinct from registry definition ids, so source
structure is resolved by conflict-checking exact description lookup. The reviewed recovery
bridge is intentionally narrower still: active effective Vortex clan `45` with catalog bonus
id `43` and the exact recovery description resolves to registry definition `577`; catalog
ability ids `729` and `1418` resolve only to their identically numbered ability definitions.
Catalog id `43` is not a registry id, and the same-text registry aliases (`577`, `729`,
`1418`, `2475`) must not collide into text-only admission: source kind, effective clan,
catalog id, registry id, description, and exact structured shape all agree or strict
construction rejects the draw. The Riots bridge applies the same rule: active effective
Riots clan `49`, catalog bonus id `47`, and the exact Victory-or-Defeat description resolve
only to registry definition `1034`. Printed catalog abilities `1375`, `4111`, `5085`, and
`5520` resolve only to their identically numbered registry definitions. A captured
`ability:1034` is executable replay evidence for a concrete post-Copy result, but no printed
catalog ability owns that id and strict construction never synthesizes it. Same-text aliases
therefore remain distinct provenance and cannot enter through description matching.
Argos follows the same fail-closed rule without an alias bridge: only catalog ability id
`1158`, exact description, and exact registry definition `1158` produce its typed capped
post-round plan; level 1 remains absent. A selected night variant has no catalog numeric id
unless the catalog explicitly supplies one; its public
identity records `None`, while the compact plan uses the resolved registry definition id.
Dynamic Oblivion Copy, global effects, unsupported temporal effects, and all other
uncompiled sources fail closed. Provenance combines the effective-catalog source fingerprint,
registry schema and source fingerprint, compiler/policy revision 12, and catalog-context
policy revision 2.

The complete 322-game replay-ready corpus supplies a construction oracle: 2,576 card slots
were derived using only catalog clans, explicit night state, and these Oculus rules. There
are 38 Oblivion slots; all 18 description mismatches are captures where the server had
already replaced printed `Copy: Opp. Ability` with the opponent-dependent copied result.
Every one of the other 2,538 slots matches captured bonus presence and description exactly.
Capture `877636` is the first current eight-card draw wholly executable by this deliberately
narrow projection. Its complete four-round server record now pins strict end-to-end catalog
construction, engine execution, and advisor replay; synthetic catalog hands continue to pin
the individual construction boundaries. This is solver-ready input construction, not full
TypeScript search-policy parity.

### 4. Port current solver semantics

The first current-engine vertical slice landed on 2026-09-17. The
`urban-recreation-advisor` binary loads canonical card/effect data through the strict
catalog boundary, enumerates every legal current-round card/pillz/Fury pairing, resolves
each sample with `CombatStatDiagnosticV1::make`/`unmake`, and renders a bounded ANSI/plain
terminal ranking. The default supported demo evaluates 8,464 pairings in a few milliseconds
in a warmed release build. First- and second-mover matrices, deadline cutoffs, deterministic
visible-percent ranking, root restoration, clipping, and ANSI/plain equivalence are tested.

That first checkpoint was usable, not solver parity. It used a clearly labelled one-round
position heuristic after nonterminal rounds, sampled current hidden choices uniformly,
accepted manual exact-card input, and only admitted
the engine's current fail-closed effect subset. Do not compare its displayed score or
runtime with the TypeScript continuation-policy solver as though they were the same
algorithm.

The second vertical slice adds a manual four-round session and exact late-game policy.
`--interactive` retains committed rounds in the real engine, alternates the explicit first
mover, and requests the revealed opposing card before second-mover advice. From round 3,
nonterminal samples recurse to exact win/draw/loss values with the same essential
information-set rule as `Policy.ts`: our response can vary by visible card but not by hidden
pillz or Fury. Cancellation unwinds every made round before returning. Rounds 1–2 keep the
bounded heuristic by design; an effect-free twelve-pill upper count is roughly 69 million
paired histories from round 2 versus about 210 thousand from round 3, before effect-driven
resource growth.

The first server-backed advisor path now loads capture `877636` with `--replay 877636`.
It derives both exact hands, resources, night state, recording side, and each round's mover
from the normalized capture; rejects any capture/catalog source-identity disagreement; and
requires complete server card evidence. Before every recorded move it renders the same TUI
and grades that move against the current ranking. It then commits the actual pair of moves
and checks power, damage, attack, winner, life, and pillz before advancing. This covers all
four rounds and both FIRST and SECOND information sets, while retaining the labelled
rounds 1–2 heuristic and exact rounds 3–4 policy. It is captured replay, not yet the live
capture stream or full TypeScript opening/round-2 policy.

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
