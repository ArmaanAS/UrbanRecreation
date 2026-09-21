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
| Candidate engine and solver backend | `rust/` | Narrow advisor integration is gated by strict supported-input and result validation; it does not establish replay or policy parity. |
| Old Rust assets and 10,000-case corpus | Historical baseline only | Useful for detecting accidental behavior changes, not evidence of current game correctness. |

The 48 TypeScript replay mismatches are known gaps in the reference, not expected Rust
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
attack, winner, life, and pillz. Captures that stopped mid-match remain classified rather
than silently discarded. Dojo (battle rule 6) captures are ordinary replay members as of
2026-09-19; they were excluded on an assumption the captures disproved.

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
Replay provenance records compiler/policy semantic revision 33 for the current scope.

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
A mismatch rejects the strict draw. Compiler/policy provenance is revision 33.

Semantic revision 13 adds only exact unconditional `Stop Opp. Ability`. The registry shape
must be player-targeted `stop_ability` with neutral attribute/action, zero control values,
no condition, link, Support, inversion, or permanence, and the literal description. The
malformed same-text id `877` and every Courage/Reprisal/Confidence/Revenge/Unison variant
remain unsupported and reject if selected. The hot resolver now tracks Ability and Bonus
liveness independently and resolves SOA/SOB dependencies in the TypeScript PRE4 order:
discard stopped controls, execute a control with no pending opposing blocker, and use stable
P1/Bonus-first order only for a true cycle. A stopped source contributes no stat modifier,
cancellation, control, or post-round work; liveness stays stack-local and make/unmake remains
allocation-free.

Strict catalog construction pins active Roots `(clan 29, catalog bonus 28)` to captured
registry definition `41` and GHEIST `(clan 32, catalog bonus 32)` to definition `94`; static
abilities retain their own exact catalog/registry id. The same context revision also pins
active Piranas `(clan 42, catalog bonus 40)` to captured Stop Bonus definition `333`, which
keeps replay source identity strict rather than treating a structural alias as execution
authority. Catalog-context policy is revision 3. Server prefixes `1088323/2` and
`1089001/1` independently pin Ability- and bonus-origin SOA arithmetic, while complete
capture `1024673` adds two sequential verified rounds. The immutable combat-stat gate is now
52 rounds. `deno task rust:advise --replay 1024673 --plain` grades both recorded decisions,
verifies both server rounds, and completes the round-two KO through the release TUI.

Semantic revision 14 adds two narrowly typed losing-side Life plans without turning all
Life text into a general resource-effect implementation. An uncapped `Defeat: +{N} Life`
Ability is admitted only with its exact positive literal and the neutral captured shape:
own Life increase, `currentRoundRequirement=lose`, `valueMin=1`, and no cap, link,
permanence, Support, special action, or other condition. It runs after damage only when
its owner survived that damage. Eugene in `877533/1` proves `14 - 4 + 2 = 12`, and Daqun
in `1092369/0` independently proves `12 - 6 + 3 = 9`. Chadwik in `1069193/3` is stopped
by Spidee's Reprisal Stop Opp. Ability and ends `18 - 6 = 12`, proving ordinary Defeat
Life remains subject to source liveness.

Reanimate is deliberately separate: only the observed `ability:4951`, Lobo level 3's
`Reanimate: +2 Life`, is admitted. It uses the otherwise identical losing-side Life shape
with `valueMin=0`, and may add Life from zero before match status is calculated. In
`1130654/1`, Lobo loses on 7 Life to Miyo's 5 damage and ends on 4, so it is not a
lethal-only trigger. In `1080877/2`, Lobo loses from 13 to Spidee's 6 damage while
Spidee's Reprisal Stop Opp. Ability is live; Lobo ends on 8, which is the already-latched
Campbell Heal +1 only (an active Reanimate would leave 10). Capped Defeat Life,
Defeat Life-and-Pillz, and other Reanimate identities remain deferred. The Kubra
Life-and-Pillz KOs in `876712` and `877023` remain the evidence boundary for suppressing
ordinary post-KO resource gains.

Semantic revision 15 admits only the captured Reprisal `Stop Opp. Ability` aliases
`ability:1310` (Spidee level 4) and `ability:2073` (Bulza Cr level 3). Their structured
source is player-targeted `stop_ability`, with neutral attribute/action and zero control
values, and uses the existing `OwnerMovesSecond` predicate. Captured arithmetic at
`1069193/3` has Spidee moving second and Chadwik ending `18 - 6 = 12`, consistent with
stopped Defeat Life; it is not an exact sequential gate because `1069193/0` first selects
Komboka's deferred `+1 Pillz And Life` bonus (`1714`) and mismatches Life 16 vs 15. In
`1060199/2`, Spidee moves first, so Reprisal is false and Donna Black's active Revenge
reduction remains in the server's Attack 44. Carmen `964`, Harmonia `1115`, Spidee level 3
`4394`, Leone Cr `1337`, and Jax Draven `5762` have the same text but remain selected
hazards, as do malformed and all other conditional SOA shapes.

Catalog admission requires the printed catalog id to belong to the resolved registry
definition's structural alias set; description equality never transfers these effects to a
different card. For this slice, that set is exactly `[1310, 2073]`, retained in provenance
alongside the actual catalog source id. Replay preparation retains every malformed near-miss
as a selected hazard. Full sequential diagnostics verify `1060199/3` and `1081463/4` with
every server round exact; because `1081463/1` was already gated, the immutable gate rises
from 52 to 58 rounds. This is stronger replay evidence, but not a claim that every strictly
catalog-eligible draw is solver-safe beyond the narrow projection. Provenance records
compiler/policy semantic revision 15.

Semantic revision 16 admits only Komboka's exact Bonus `1714`, `+1 Pillz And Life`, on an
active effective Komboka clan (`54`) card: `value=1`, zero bounds/condition,
`position=both`, `currentRoundRequirement=win`, player-targeted `life&pillz` increase, no
special action, link, Support, permanence, or other context. The compact plan is a single
typed post-round effect. Its TypeScript-compatible
commit is atomic and ordered Pillz then Life; it runs only for a live winning source, while a
loss (including a lethal loss) receives neither payment nor revival. `1069193/0` now exactly
verifies Pantherine's 16 Life and 8 Pillz endpoint and raises the immutable gate from 58 to
59 rounds. `1069193/1` next selects Hilal's deferred `ability:5355`, `-1 Opp. Pillz And
Life, Min 0`, so no longer prefix is claimed.

Server arithmetic supplies additional bounded evidence without claiming unsupported prefixes:
Hewa Cr's `866431/0` win ends 13 Life/6 Pillz but also selects deferred Support Life;
Keya's tied `876882/0` win is stopped by Courage Stop Bonus and ends 12/12; Adytia's
`1066077/1` win ends 8/5 while Spidee's SOA leaves the Bonus live; Kunglaba's KO win in
`1023608/3` ends 12/1 after payment; and Kubra's lethal `876712/1` loss ends 0/9 with no
payment. Same-text `ability:3356`, Kubra `ability:1716`, malformed `1714`, and all other
compound or capped variants remain selected hazards.

Semantic revision 17 adds a closed Victory-or-Defeat Life family without widening generic
Life-effect admission. Exact registry definitions `1396`, `2992`, `5835`, and `5799`
produce `+1` own Life; `2944` and `5802` produce `+2`; definition `1628` produces
`-1` opponent Life, Min 1. Description, magnitude, minimum, target, operation, neutral
condition/link/permanence fields, and registry identity must all match. Captured Copy may
materialize one of those reviewed definitions as either an Ability or Bonus, so replay
preparation preserves that source kind for Stop Ability/Stop Bonus liveness. Strict catalog
construction remains narrower: only the reviewed canonical card/level/catalog-id mappings
are admitted, with Zerkov's catalog-local `5800`/`5801` levels explicitly resolving to the
captured `5799` definition. Dynamic catalog Copy and Scott Ld level 2's uncaptured `5834`
identity remain fail-closed.

The typed END-phase plans run after damage in stable P1/P2, Bonus-before-Ability order. A
living owner receives its own Life gain after either outcome, but ordinary Victory-or-Defeat
Life is not Reanimate and cannot revive a KO. Opponent reduction can still run after its
source owner was KO'd, skips a target already at zero, and clamps a living target at 1.
Checked own-Life addition is atomic, and make/unmake restores the complete position without
new latch state or hot-path allocation. Server captures independently cover own-Life wins,
a clean Kora Mail Ld loss, Stop Opp. Ability suppression, opponent reduction on both
outcomes, and reduction after the source owner's KO. The unobserved own-Life owner-KO and
exact Min-1 boundary follow the stable TypeScript reference and are pinned synthetically
rather than presented as live-server observations.

Capture `925719/3` supplies a complete new sequential gate: Uuber loses round 0, takes
Prince Jr's three damage, and still reduces the live opponent from 12 to 11; the following
two rounds also remain exact. This raises the immutable diagnostic gate from 59 to 62
rounds and makes `925719` the eighth strict complete draw.

Semantic revision 18 adds one selected-opponent-star post-round family without widening
generic Equalizer or Life admission. Registry definitions `1415` (Hal Gladius level 2) and
`4458` (Gail Ld level 2) must exactly describe `Equalizer: - 1 Opp. Life Min 2`: value one,
Min 2, `currentRoundRequirement=win`, opponent Life decrease,
`isOppStarsLinked=true`, and every other condition, link, Support, permanence, and special
field neutral. Replay preparation also accepts either reviewed identity in a Bonus slot,
because capture `924669` materializes `1415` there through Copy. Strict catalog construction
remains narrower: only the two printed ability card-key/catalog-id pairs are authoritative;
Gail level 1 `5536`, O Riley `4455`/`4125`, El Cazador `5793`, same-text sources, catalog
bonuses, and dynamic Copy remain fail-closed.

After Stop Ability/Stop Bonus liveness is resolved, preparation multiplies one by the
selected opponent card's immutable level and emits a fixed, allocation-free END plan. A
winning source applies it after normal damage. It changes the live target only when current
Life is above Min 2, so zero, one, and two remain unchanged; it cannot revive a KO or raise
one Life to two. Direct server arithmetic covers opponent levels one through five
(`943111`, `924740`, `963694`, `949750`, `970972`), with separate observations for the
Min-2 clamp, SOA suppression, SOB isolation, and copied-Bonus execution. Life-modifier
cancellation remains outside this slice and therefore keeps affected solver draws closed.

The executable sequential gate adds `943111/1`, `924740/2`, `963694/1`, and `970972/1`,
and extends `877812` from two to all three rounds, raising it from 62 to 68 rounds. The
four-star, Min-2, SOA, and Copy captures have unrelated earlier or selected unsupported
effects, so they remain focused evidence instead of being misrepresented as sequential
gates.

Semantic revision 19 adds one identity-locked Courage conversion without widening generic
Life, conversion, or same-text admission. Only Anita `(card id 448, level 3)` with printed
Ability/catalog/registry id `274`, exactly `Courage: +1 Life Per Dmg`, and the complete
reviewed structured record is executable. The record requires value one, attacker position,
a current-round win, player Life increase, `convert_dmg_to_life`, and otherwise neutral
bounds, conditions, links, flags, Support, and permanence. Anita levels 1–2, Ellie and
Lorea's same text, Bonus provenance, dynamic Copy, other cards, and malformed id-274
records remain fail-closed.

After ordinary damage resolution, a live winning Anita gains her final resolved damage as
Life only when her owner moved first. This uses the engine result after Fury and combat
damage modifiers, not the printed damage or capture transport metadata; it cannot revive a
KO. The checked addition is part of the private replacement position, so overflow aborts
atomically, make/unmake restores the exact prior state, and the hot path adds no allocation.
`1059454` supplies a normal Courage win, `1089346` supplies a Fury win with resolved damage
five and final Life 17, and `1069813` proves a live Courage loss gains nothing. Capture
`875375` separately pins the transport boundary: its replay result remains damage three
even though `damageAfter` is five.

The immutable sequential gate adds 19 unique rounds through `1059454/4`, `1069813/4`,
`1078906/3`, `1089346/4`, and `1090607/4`, raising the gate from 68 to 87 rounds.
`1061897/4` was already present in the prior gate; revision 19 changes Anita's disposition
there from disabled to executable but does not double-count those four rounds.

Semantic revision 20 adds the first unconditional Victory opponent-Life reductions without
widening generic Life admission or admitting any conditional sibling. Exactly two reviewed
identities are executable: Mou level three's printed `ability:1399`, `-5 Opp. Life Min 5`,
and the active Berzerk clan `bonus:680`, `-2 Opp. Life Min 2`. Each structured record must
carry its exact value and matching minimum, `currentRoundRequirement=win`, an opponent Life
decrease, and otherwise neutral position, index, condition, link, Support, special-action,
multiplier and permanence fields. The source kind is authority in both directions: `1399` is
only ever an Ability and `680` only ever a Bonus.

Execution reuses the existing typed `ReduceOpponentLifeOnVictory` END-phase plan. It runs
after ordinary damage for the round winner only, changes a live target only while its Life is
still above the bound, and clamps at that bound rather than below it; it cannot revive a KO
and adds no hot-path allocation. Ordinary Stop Opp. Ability and Stop Bonus liveness still
suppress it.

The catalog boundary is narrower than the registry. Mou must be the exact card key with
printed ability id `1399`, and Berzerk resolves only through active effective clan `46` with
catalog bonus id `44`, matching the existing Roots, GHEIST, Vortex, Riots and Jungo bridges.
The three same-text catalog ids — Rakhan `978`, Milovan `498` and Fraser `1289` — have no
registry definition at all and therefore stay fail-closed without special handling; dynamic
Copy and every other card remain rejected.

Server arithmetic covers both magnitudes and the bound. Mou takes 12 to 5 on two damage in
`1091473/0`, and `926367/1` proves the clamp: post-damage 6 becomes 5 rather than 1. Focused
evidence in `924257/3` shows a target already below the bound left untouched, while the
already-gated `945585/1` keeps an inactive losing Mou. Berzerk takes 12 to 6 on four damage
in `877093/0` and 14 to 10 on two damage in `1080662/1`, while `1025102` supplies three
consecutive Berzerk defeats that must pay nothing at all. The server independently names this
effect: the capture transport's own `postRoundAbilities` records a quantity-2 Life decrease in
`1025645/2`, `1080662/3`, `1092020/3`, `876752/1` and `877533/3`.

The immutable sequential gate adds `926367/3`, `1091473/2`, `877093/1`, `1080662/2` and
`1025102/3`, raising it from 87 to 98 unique rounds. Revision 20 left the conditional
siblings — Symmetry `4708`, Courage `4533`, Confidence `3016` and Growth `1730` — explicitly
deferred, believing a post-round plan carried no predicate. Revision 22 found that it already
did and admitted `4708`, `3016` and `4301`; `4533` and `1730` remain deferred for the reasons
recorded there.

Semantic revision 21 adds unconditional Copy to the strict catalog constructor. Only the
exact texts `Copy: Opp. Ability` and `Copy: Opp. Bonus` are admitted, and like generic
Victory Life they are admitted by grammar rather than a fixed identity list, because the
registry carries many structurally identical Copy definitions. The structured record must be
completely neutral apart from `copy_ability` or `copy_bonus`. The printed catalog id must
itself be a registry definition of that exact text and shape, so a printed Copy whose catalog
id is not a definition — Lorna's `752`, for instance — stays fail-closed, and description
alone never admits. Every conditional variant (Reprisal, Revenge, Asymmetry, Unison,
Confidence, `Bet > N`) and every stat-copying variant (`Copy: Opp. Power`, `Copy: Opp.
Damage`, `Copy: Power And Damage Opp.`) keeps its own deferred grammar.

The capture format establishes the rule directly. Ability identity lives only in a battle's
static block, and the capture writer emits a new one whenever the server rewrites it, so a
Copy card's resolution is recorded exactly once, at the round it fires: 53 unconditional
resolutions are visible across the corpus. `Copy: Opp. Bonus` adopts the opposing selected
card's bonus and `Copy: Opp. Ability` its ability, and a single card may carry both
(`1025470` round 1).

Three semantics are server-established rather than assumed. The adopted effect belongs to the
copier: eight rounds copying Rescue's `Support: Attack +3` all scale by the copier's own
effective-clan count, including `1078906` round 1 at four Riots for Attack +12 and
`1080007` round 0 at two for +6. `1081688` round 0 confirms the count is the effective clan
rather than the printed one: Dark Dalhia infiltrates the singleton Komboka, giving two and
+6. The adopted effect also keeps the copier's own slot kind, so `874590` round 1 shows an
opposing Reprisal Stop Opp. Ability suppressing a copied bonus entirely. A bonus-origin
effect therefore executes as the copier's ability, which `1093500` round 0 already proves
for Riots' Victory-or-Defeat Pillz.

Because a solver must be total over every legal selection, a Copy is admitted only when every
opposing card it could face already carries a concrete adoptable plan. A Copy of a Copy, of a
disabled source, or of a selected hazard closes the whole match at construction rather than
deferring failure to the round that would have to resolve it. An absent opposing source is
concrete: the Copy adopts nothing. Resolution stays allocation-free and string-free, because
the opposing plan is already materialized and adopting it is a plain index; make/unmake is
unchanged.

Semantic revision 22 adds a predicate to the two post-round families whose machinery already
existed. `active_effect` had always run `predicate_matches` over a source's post-round effect
as well as its combat effect, so nothing in the engine needed a new mechanism: only admission
was closed. The compact `CopyOpponentSource` plan now carries a predicate that gates the
adoption itself, and both preparation dispositions expose the predicate beside the effect, so
a conditional post-round plan can no longer read as an unconditional one.

Conditional Copy admits exactly four more grammars — `Reprisal: Copy Opp. Bonus`,
`Reprisal: Copy: Opp. Ability`, `Revenge: Copy Opp. Bonus` and `Revenge: Copy: Opp. Ability`
— by exact description and structured shape, like unconditional Copy. Note the site's own
inconsistent punctuation: a copied Bonus loses the second colon under these prefixes while an
Ability keeps it. Exactly one structured field may carry the condition and it must be the one
the printed prefix names, so a Reprisal record with `previousRoundRequirement=lose`, or a
Revenge record with `positionRequirement=defender`, fails closed. The condition gates only
whether the opposing plan is adopted; the adopted plan then keeps its own predicate, so a
copied Confidence effect must satisfy the copier's history rather than inheriting the
original owner's. Totality is unchanged: a predicate depends on the round, not the draw, so
every conditional Copy has some legal line in which it does adopt, and one unresolvable
opposing target still closes the whole match at construction.

The server evidence separates the two branches cleanly, using the same capture property the
unconditional slice relies on: a Copy's static block records what it resolved to. A Reprisal
Copy therefore keeps its printed text when its owner moved first — `1060341/3`, `1091235/3`
and `1091381/3` — and shows the adopted source when it moved second, in `1088323/2` (McMaster
takes Sue's `916`), `943111/2` (an adopted Stop Opp. Ability), `1059030/1`, `1066077/0`,
`877167/1`, `926584/1`, `1079482/2`, `1092992/0`, `1059454/0`, `1090607/0` and `1130425/0`.
Revenge's decisive round is `1059149/3`: Cravy adopts Rescue's `Support: Attack +3` after
losing round 2, and the server's attack of 40 is 7 x 4 plus 3 x **4** — scaled by Cravy's own
four clan-mates, which extends the already-established "the adopted effect belongs to the
copier" rule to the conditional form.

Two captures qualify that reading and are recorded rather than smoothed over. In `1069721/3`
and `875098/2` the condition held, yet the static block still carries the printed Copy text.
In both, the ability that would have been adopted is a Confidence effect whose own predicate
was false for the copier, so nothing observable differed either way. The rule that survives is
narrower than "the capture always shows the adopted source": a static-block rewrite is
evidence of adoption, but its absence is not evidence against it. The model is unaffected,
because both rounds resolve identically under it.

Conditional Victory opponent-Life admits three more reviewed identities on the existing typed
`ReduceOpponentLifeOnVictory` plan: `4708` (Doela Noel level 2, Symmetry, -4 Min 0) and the
byte-identical pair `3016` / `4301` (Diabolus levels 2 and 1, Confidence, -3 Min 0), each
admitted on its own evidence rather than one being treated as an alias of the other. As
everywhere else in this projection, each identity carries exactly one permitted predicate, so
a plan can neither add a condition to an unconditional member nor swap another in.

Symmetry has three independent active observations and a clean inactive one. `1011297/0` and
`877308/0` both put Doela Noel and her opponent in hand slot 1: two printed damage takes 12
Life to 10 and the complete -4 makes 6. `1023274/2` is a Fury win on matching slot 0 where the
capture transport independently names the effect, recording a quantity-4 non-permanent Life
decrease in `postRoundAbilities`. `1058151/2` is the inactive branch on unequal slots: the
Asymmetry bonus fires instead, damage is 5 rather than 2, and 8 Life becomes 3 with no
reduction at all. Confidence has two observations in independent captures: `948654/2` takes 10
to 8 on two damage and then to 5, and `878056/2` takes 6 to 5 on one damage and then to 2,
both after their own side won the preceding round. Its inactive branch has no captured
observation and is pinned synthetically.

`1011297/0` is also a valid sequential prefix and raises the immutable diagnostic gate from 98
to 99 rounds: Aneta's Courage leaves Doela Noel at 6 power for attack 42 after Edd Cr's clan
reduction, and every server field is exact. The other four observations sit behind an
unsupported effect earlier in their own prefix — Pride's `Protection: Attack` in `1011297/1`,
Edd Cr's Courage Stop Opp. Bonus in `877308/0`, a Poison bonus in `1023274/0`, Malfass in
`948654/0` and a Heal permanent in `878056/2` — so they remain focused arithmetic evidence
rather than gate members.

Two members of the family the measurement grouped together are deliberately left out. Courage
`4533` (Ligea level 3) has **no selected observation anywhere in the 359-capture corpus**, so
there is nothing to check it against; the Min-1 siblings `4531`/`4532` that do appear are a
different effect. Growth `1730` is a round-scaled magnitude rather than a predicate, which is
the separate design step a post-round plan still cannot carry. Both are pinned as fail-closed
under the exact structure they share with the admitted members, which is the point: admission
here is by reviewed identity, never by shape.

Catalog admission stays narrower than the registry, as usual. Only the exact card keys and
printed ids are authority — Doela Noel `(2058, 2)` with catalog ability `4708`, Diabolus
`(2270, 1)` with `4301` and `(2270, 2)` with `3016`. Doela Noel level 1 prints the same text
under catalog id `4843`, which is not a registry definition at all, so it stays fail-closed
with no special handling, exactly as Rakhan, Milovan and Fraser do. The same rule closes Nexus
and XU91, which print an admitted Copy text under catalog ids `1334` and `1828` that own no
definition of it.

Strict catalog coverage rises from 14 to 17 of 328, adding `878011`, `925254`, and
`1078906`. None of the three is advisor-replayable, and that is deliberate: a Copy card's
captured static block records what the Copy resolved to rather than the printed Copy, so
capture and catalog identity cannot agree. `--replay` and the hosted worker both refuse those
draws with that exact reason instead of preferring either side. Revision 21 left the 98-round
sequential gate unchanged, because replay preparation continues to consume each capture's
recorded resolved source and never emits a Copy plan.

Replay preparation scans all eight cards. Canonical Leader clan id 36 and Team/global or
Mock/Illusion sources are fatal even when unplayed, because they may execute off-card.
Unsupported card-local controls and every unadmitted current-round combat-stat modifier are
retained as visible Disabled metadata but reject atomically if selected. The 33 observed
ordinary Support combat-stat definitions are admitted only for an otherwise-neutral,
unconditional basic Attack, Power, or Damage shape. Conditional, nested, life, pillz,
Power-and-Damage, post-round, and permanent Support effects remain fail-closed; every capped
increase except exact `ability:1158` remains deferred. Only the exact audited
Victory-or-Defeat Pillz and Life identities are admitted; all other same-text sources remain
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
context. Compiler/policy revision 33 records this semantic boundary together with the exact
Equalizer multiplier, the bounded Confidence/Revenge/Frozn slice, the identity-locked
Defeat-recovery post-round plan, the identity-locked Victory-or-Defeat Pillz family with its
both-owner post-round execution semantics, Argos' identity-locked capped Defeat gain, and the
exact-structured positive Victory Life and Victory-or-Defeat Life plans, the identity-locked
Equalizer opponent-Life plan, Anita's identity-locked final-damage Courage conversion, the
two reviewed unconditional Victory opponent-Life reductions, and the four latched plain
permanent Life grammars.

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
`5520` resolve only to their identically numbered registry definitions. No printed catalog ability owns id `1034`, so strict construction never
materializes it as a card's own printed source. Since revision 21 it can still reach a card
through an unconditional Copy, which adopts the opposing selected card's actual plan rather
than synthesizing an identity; capture `1093500` round 0 is the server evidence for exactly
that. Same-text aliases remain distinct provenance and cannot enter through description
matching.
Argos follows the same fail-closed rule without an alias bridge: only catalog ability id
`1158`, exact description, and exact registry definition `1158` produce its typed capped
post-round plan; level 1 remains absent. A selected night variant has no catalog numeric id
unless the catalog explicitly supplies one; its public
identity records `None`, while the compact plan uses the resolved registry definition id.
Conditional Copy, global effects, unsupported temporal effects, and all other
uncompiled sources fail closed. Provenance combines the effective-catalog source fingerprint,
registry schema and source fingerprint, compiler/policy revision 33, and catalog-context
policy revision 3.

The complete 322-game replay-ready corpus supplies a construction oracle: 2,576 card slots
were derived using only catalog clans, explicit night state, and these Oculus rules. There
are 38 Oblivion slots; all 18 description mismatches are captures where the server had
already replaced printed `Copy: Opp. Ability` with the opponent-dependent copied result.
Every one of the other 2,538 slots matches captured bonus presence and description exactly.
On 2026-09-18, the deterministic strict-coverage regression scanned all 359 captured
complete 4+4 hands with canonical `data/data.json`, battle-card overrides, captured
`abilities.json`, and each capture's rule, night, life, and pillz context. It constructs
`CatalogCombatStatMatchV1` under `RequireFullyExecutableDraws`. The eligible set is pinned
there and holds 69 capture ids at revision 34; the list below is the 61 it held at revision
25, kept as the base the per-revision additions after it are stated against: `830285`, `869944`, `874520`, `875098`, `875155`, `875322`, `877636`,
`877687`, `877773`, `877812`, `877860`, `877950`, `878011`, `878056`, `924257`, `924320`,
`924413`, `925254`, `925674`, `925719`, `925796`, `943111`, `946112`, `947228`, `949750`,
`956608`, `970972`, `1011643`, `1011712`, `1023274`, `1024673`, `1025102`, `1025525`,
`1058366`, `1059030`, `1059454`, `1060052`, `1060199`, `1061897`, `1065812`, `1069813`,
`1070101`, `1070207`, `1072715`, `1078906`, `1079482`, `1080877`, `1081463`, `1089121`,
`1089346`, `1090607`, `1091235`, `1091585`, `1092294`, `1092369`, `1092454`, `1092909`,
`1092992`, `1093399`, `1130577`, and `1130833`.
Revision 21 added `878011`, `925254`, and `1078906` to revision 20's fourteen; revision 22
then added `875098`, `875322`, `1011712`, `1059030`, `1059454`, and `1090607`; revision 23
added `877687`, `924257`, `1070207` and `1091235`, the four the report had predicted for
Protection; revision 24 added `943111`, `946112`, `947228`, `1065812` and `1092909`, the
five predicted for the two Copy families together; revision 25 added `874520`, `925796`,
`949750`, `1058366`, `1060052`, `1072715` and `1130833`, the seven predicted for Attack per
opposing Damage and Defeat opponent-Life; revision 26 added `877773`, `877860`, `878056` and
`1079482`, the four the blocker-set listing had attributed to `3526` alone; revision 27
added `924320` and `1080877`, the two a candidate-family line for the plain Heal grammar
had measured before it was admitted; revision 28 added `875155`, `1091585`, `1092294` and
`1092369`, the one measured for plain Toxin and the three for plain Poison; revision 29
added `1092454` and `1092992`, the two measured for plain `+N Pillz`; revision 30 added
`924413` and `956608`, the two measured for plain `-N Opp Pillz. Min M`; revision 31 added
`1011643`, `1023274`, `1025102` and `1025525`, the four measured for Pillz per Damage;
revision 32 added `1070101`, `1089121`, `1093399` and `1130577`, the three measured for
uncapped Life per Damage and the one for the predicate-carrying permanents; revision 33
added `926367`, `962243`, `963039` and `1069506`, the two measured for the plain Victory
`-N Opp. Life, Min M` grammar and the two for its Victory-or-Defeat form; and revision 34
added `1010898`, `1011183`, `1130609` and `1131010`, the two measured for the capped
Life-per-Damage forms and the two for predicate-carrying fixed Victory Life.
That is six for two families the report had predicted would unlock three each, because six
draws were blocked by *both* families at once — which is exactly why reach and unlock are
measured separately, and why the measurement has to be rerun rather than added up. Catalog
eligibility is a strict whole-draw admission measurement, not proof of full engine or
TypeScript solver parity;
the 299-round immutable diagnostic gate supplies the separately checked sequential replay
evidence. Synthetic catalog hands continue to pin individual construction boundaries.

Semantic revision 23 adds Protection, the projection's first defensive control channel.
Three printed grammars are admitted by exact text and structured shape, like Copy and
generic Victory Life rather than by an id list, because the registry carries many
structurally identical records of each: `Protection: Power And Damage` (`759`, `880`,
`1355`, `1464`, `1793`, `2295`, `3232`, `3550`, `5761`), `Protection: Ability` (`461`) and
`Protection: Bonus` (`481`, `1132`, `1515`, `1554`, `2860`, `4098`, `4983`, `5498`). The
structured record must name the owning player, carry no magnitude and be otherwise neutral.
`Protection: Power`, `Protection: Attack`, the site's spaced `Protection : Damage` and the
clan-conditional `After [clan:25]: Protection : Damage` all remain Disabled: each is a
different grammar and none has a reviewed round behind it.

The two halves are separate mechanisms. A protected stat refuses an opposing decrease: it
removes nothing and reorders nothing, so the descending-Min ordering of the reductions that
do apply is untouched. Nebula keeps 7 Power against Olga Cr's `-2 Opp Power, Min 5` in
`949439/0`, keeps 4 Damage against Donald's `-3 Opp Damage, Min 2` in `924320/1` and against
Henry's Support reduction in `942983/2`, and Miss Pandora keeps 7/4 against Sue's `-1 Opp
Power And Damage, Min 3` in `1069506/0`. Only reductions are refused; nothing in the corpus
has an opposing increase to refuse, and the projection does not admit one.

A protected source survives an opposing Stop. In `926525/0` Lumia Cr's Stop Opp. Ability
does not stop Andy Ld, whose `-20 Opp Attack, Min 5` takes Lumia Cr's own 36 Attack to the
16 the server reported, because the Skeelz `Protection: Ability` bonus is live. Two further
rounds outside the strict gate say the same for each half: `876752/0` has Lady Ametia Cr
reach 13 Power through Mavi's Stop Opp. Ability, and `964088/1` has El Tortillo keep `+1
Attack Per Life Left` through Miyo's Stop Opp. Bonus for an attack of 60.

Protection resolves after the Stop graph, which is where TypeScript applies it too: cancels
run at PRE4 and Protection at PRE3, and `blocked` is `cancel && !prot`. Two consequences are
deliberate and pinned rather than inferred. A protecting source that was itself stopped
protects nothing, which is what keeps a self-referential Protection inert. And a source that
Protection restores keeps its combat effect but has already missed the Stop graph, so it
does not retroactively stop anything. Neither has captured evidence; both are TypeScript
parity, and `rust/tests/combat_stat_diagnostic_engine.rs` names them as such.

The older `ClanBonusDiagnosticV1` projection is deliberately untouched. It has no liveness
model, so a Protection source stays a disabled card-local source there exactly as exact
`Stop Opp. Ability` does, now by an explicit branch rather than by the registry failing to
compile it.

Semantic revision 24 takes both Copy families the previous measurement named, after
splitting them further and measuring the pieces: unconditional stat Copy unlocks 3 draws,
Asymmetry source Copy 2, and together 5, because they share none. Unison Copy, conditional
stat Copy and Exchange each unlock 0 and are left closed.

A stat Copy replaces the owner's own value with the opposing selected card's **printed**
value, before any increase of its own and before any opposing reduction. Three grammars are
admitted by exact text - `Copy: Opp. Power`, `Copy: Opp. Damage` and the site's own word
order for the pair, `Copy: Power And Damage Opp.` The structured record must write to the
owning player and carry no magnitude. `Power Exchange` and `Damage Exchange` use the same
`copy` action with `sideAffected: both` and swap the two cards' values instead, so they are
refused by side rather than by grammar, and every conditional prefix is a different
description and refused with it.

The server pins each part. `1025031/0` settles both halves at once: Natasha copies
Nantosuelte's printed 4 Damage - not the 7 its Asymmetry bonus had already made of it - and
her own `Damage +2` then produces the reported 6. `1065812/1` shows the copy landing before
an opposing reduction, Joana taking Sue's printed 6 Power for the reported 5 after Sue's own
`-1 Opp Power And Damage, Min 3`. `1069345/0` does the same for the pair grammar, and
`876635/1` is the plain case, Javert at Keya's printed 8 Power. `1023946/1` and `1093079/3`
agree outside the gate. Because both sides read printed values, two simultaneous copies
cannot depend on which one resolves first.

Asymmetry source Copy adds two grammars to the existing Copy table, `Asymmetry: Copy: Opp.
Ability` and `Asymmetry: Copy: Opp. Bonus`, with the `SelectedHandSlotsDiffer` predicate the
projection already resolves. The shape check now requires each predicate's own structured
field, so `indexRequirement=asymmetry` is what an Asymmetry prefix must carry and a Reprisal
or Revenge record cannot borrow it; the clan-gated `Asy.` variant `5073` still fails on its
clan requirement, and `Unison` remains deferred because its condition is a draw-level
clan-mate count the projection has no predicate for.

This half is admitted on catalog evidence rather than replay evidence, and the distinction
is worth stating. Every selected Asymmetry Copy in the corpus - `1088008/1`, `1089452/0`,
`1089742/0` and `1092909/1` - reaches its capture with the static block already rewritten to
the source it adopted, so no replayed round ever presents the Copy grammar to the engine.
What those four do confirm is the positive branch and the ownership rule: in `1089452/0` and
`1089742/0` the adopted Rescue `Support: Attack +3` scales by the *copier's* clan-mates, three
and four respectively. The negative branch - matching hand slots adopt nothing - rests on the
same structured `indexRequirement` the projection already reads for numeric Symmetry and
Asymmetry effects, and `rust/tests/combat_stat_diagnostic_engine.rs` says so where it pins it.

Semantic revision 25 takes the two families the reach table suggested and the measurement
confirmed: `+N Attack Per Opp. Damage` unlocks 3 draws, `Defeat: -N Opp. Life, Min M` 4, and
together 7 with no overlap.

The Attack conversion is an ordinary own-Attack increase with a new magnitude. The registry
keeps its link in `specialAction: convert_opp_dmg_to_atk` rather than in one of the
`is*Linked` flags, so it needs its own compile arm, and the printed text names the link
after the magnitude - `+2 Attack Per Opp. Damage` - which is checked whole rather than as a
prefix. `+N Attack Per Opp. Power` is a different special action with no reviewed round and
stays closed.

What the magnitude reads is the point. The Attack phase sees the opposing Damage as it
stands after every Power/Damage modifier and before Fury, which is exactly where TypeScript
moved it on 2026-09-17. Battle 1130726 r3 is the round that separates the two readings:
Goran's +2 is worth 4 against a Fury Uuber, not 8, for the 24 Attack the server reported
against 28. That round is not strict-eligible, so the gate cannot carry it; the engine test
does, beside a reduction case showing that an opposing Damage reduction *is* counted because
it resolves before this phase. Eleven gate rounds pin the magnitude itself, including
1066077/1, where Spidee's Reprisal Stop Opp. Ability leaves Adytia Ld at a plain 8 x 4.

Whether the multiplier is the opponent's printed Damage or its resolved Damage minus Fury
remains formally open: no captured round both modifies the opposing Damage and converts it.
This implements the resolved reading, which is what the TypeScript reference does, and
`docs/replay-triage.md` records the same limit from the other side.

`Defeat: -N Opp. Life, Min M` is the losing-side sibling of the Victory reduction and shares
its post-round channel, so it needed admission rather than machinery. It is admitted by
exact printed text and a neutral structured shape - the registry carries six structurally
identical records across four Min values - and the printed numbers are authority: a record
whose text disagrees with its own magnitude or bound is refused rather than trusted either
way. The owner having lost is the whole trigger, so an owner taken to zero still pays it
out, as the reviewed Victory Or Defeat reduction does, while a target already at or below
Min is left untouched rather than pulled up to it. 925796/0, 925999/0 and 945989/1 are three
independent losses that each pay 2; 874520/0 is the same card winning instead, which pins
the trigger rather than the magnitude.

Semantic revision 26 adds the projection's first repeating effect and admits exactly one
identity through it: Lianah Ld level 3's `Heal 1 Max. 20` (`3526`), the source the blocker
listing said held four draws on its own. The four came: `877773`, `877860`, `878056` and
`1079482`, no more and no fewer.

The mechanism is a latch, and it lives in the position rather than in the plan.
`BaseRulesPosition` carries `latched: ByPlayer<LatchedEffectsV1>`, a fixed array of at most
one permanent per card in latch order, so it is snapshotted and restored by the same undo
token as Life and Pillz and takes part in the structural hash: two positions that agree on
everything else but differ here play out differently, and a transposition key that ignored
it would be wrong. The source plan is ordinary post-round work, `HealLifeOnVictory { life,
maximum }`, bound only if the source is live after Stop resolution exactly like every other
post-round plan; the commit turns it into `LatchHealLifeOnVictory`, which writes the effect
into the winner's list and pays nothing in that round. Every later commit then walks each
owner's pre-round list after that owner's own current-round effects and before the other
owner's, which is the TypeScript reference's END order - each side's fresh effects, then its
`repeat` bucket - and pays `life` while the owner is living and below `maximum`, capped at
`maximum` and never lowering a value already above it. Reading the pre-round list is what
keeps the round's own latch out of the round's own repeats.

The server pins each part. 878093/0 has Lianah winning on 12 and the owner still on 12 after
the round; 878093/1 is then won by Buck with a KO and the owner ends on 13, so the repeat
pays whatever card is played and a match-ending round still pays it. 1091985/1 has Lianah
winning with Fury for a KO and the server reporting the permanent Life increase for that
round as quantity 0 - the latch round pays nothing even when it is the last. 877733/0 has
Lianah winning into Pr Balthazar's `Stop Opp. Ability` and the owner then sitting on 12
through round 1 and taking Agnes' full 4 in round 2: a stopped Heal never latches, which is
why liveness is judged once, in the latching round, and not re-asked of whichever card the
owner plays later. None of those three draws is strict-eligible, so the gate carries the four
eligible ones instead - in every one of which Lianah loses her round, pinning the negative
branch - and `rust/tests/combat_stat_diagnostic_engine.rs` reproduces the positive numbers,
the cap, the Stop, the order against a same-round `+2 Life`, and undo.

One choice is deliberate and unobserved: a latched owner taken to zero is not revived by
a repeat. The repeat is ordinary Life, not Reanimate, and no capture shows a Heal on a KO'd
owner. Revision 26 also admitted Lianah's identity alone; revision 27 below widens that to
the grammar. `Defeat : Heal` latches on a loss, `Asymmetry: Heal` on a hand-slot predicate,
and Poison, Toxin and Regen need latch variants of their own (opponent-targeted with a Min,
and Toxin and Regen pay in the latching round), so none of them is either slice.

Semantic revision 27 widens Heal from Lianah's identity to the plain `Heal N Max. M`
grammar, the way Victory Life and Protection are admitted: by exact printed text and
complete structured shape rather than an id list, because the registry carries eight
structurally identical records - `649`, `751`, `963`, `1501`, `3118`, `3526`, `4625`,
`5341` - whose only differences are the two printed numbers. The printed numbers are
authority, so a record whose text disagrees with its own `value` or `valueMax` is refused
rather than trusted either way, and the grammar is card abilities only because no clan
bonus prints a Heal. The compact plan can therefore require only the Ability slot, a positive
magnitude below a positive cap, and no condition; the card lock revision 26 carried is gone.
In the catalog the row's numeric id must be a structural alias of the definition its text
resolves to: Campbell level 4's `963` and level 3's `4625` both qualify, level 2's `4624`
has no captured definition and is refused although its text resolves, and a same-text row
under a foreign id cannot latch.

This is the widening revision 26 had priced as admission-only, and it was: no engine line
changed. It unlocked the two draws a candidate-family line had measured for it, `924320` and
`1080877`, and the gate grew by eleven rounds. `1080877` is the paying draw: Campbell's
`Heal 1 Max. 15` latches in round 0 and pays after each of rounds 1, 2 and 3 - beside Scott
Ld's Victory-or-Defeat Life in round 1, and in round 2 where Spidee's Reprisal Stop Opp.
Ability stops Lobo's Reanimate but not the Heal latched two rounds earlier, which is the
round the Reanimate evidence had already leaned on. `1059895` does the same for `4625`
after Campbell wins a 61-61 tie on level: round 1 pays beside a VOD, round 2 beside Cleo's
Defeat Life, round 3 after a loss. `924669/0` is a latch round paying nothing while Uuber's
VOD reduction lands, and `875375/0` and `1025525/0` are losing Heals that never latch. Every
other plain Heal round in the corpus sits behind an unadmitted grammar in the same draw.

Semantic revision 28 puts the other three plain permanents on the same latch and closes
the family's mechanism: `Regen N, Max. M` (`1458`, `3433`), `Poison N, Min M` (`206`, `325`,
`509`, `566`, `582`, `682`, `1345`, `1385`, `3088`, `3603`, `5901`) and `Toxin N, Min M`
(`1197`, `1508`, `1840`, `4730`, `5037`, `5098`, `5638`, `5639`, `5640`), each admitted by
exact text and complete structured shape like Heal. Two things needed adding. The latched
effect now carries its own kind, and the kind decides whether the latching round pays: the
structured records tell Toxin from Poison and Regen from Heal only by
`isImmediatePermanent`, which is the field the TypeScript `delayed` flag was reading all
along, so the compiler requires it to agree with the text. And the repeat loop walks the
post-latch list rather than the pre-round one, skipping only what was latched this round
and does not pay at once, so a Toxin pays in its own round in latch order behind the older
permanents. Poison is the one permanent a clan prints as its bonus, so it alone is open to
both slots, with the Freaks catalog bonus `38` bridged to the captured `206` exactly as
Jungo's `41` is to `401`; Regen and Toxin are card abilities only.

The server pins each part, and the immediacy most of all. Galactea's Toxin takes one Life
in the round it wins (1091904/1: 12 - 1 Damage - 1), Zis' takes one beside three Damage
(963039/0: 12 to 8), Dr Elisa's beside two (1090531/0: 12 to 9), and Padre Frollo's Regen
brings 5 back to the cap of 6 in the round after he wins (1059149/2), then reports 0 at the
cap (1059149/3). Poison waits: Sofilia's Freaks bonus latches in 1092369/1 and pays two in
round 2, and the bonus latched in 1023274/0 pays nothing that round and two in the next.
The floor holds from both sides - 1060510/2 leaves a target on 2 under Min 3, and the two
Freaks latches in 926420/3 report 2 and then 0 once the target sits at 3 - and so does the
cap. A latched reduction outlives its owner's knockout: Fridlia Cr's Toxin pays in
1091585/2 and Araaknat's Poison in 1092294/3 while their owners are taken to zero. And the
repeat can end the match: in 963039/2 Regan's Victory-or-Defeat reduction takes the
opponent from 3 to 1 and Zis' Toxin, latched two rounds earlier, takes the last point; Toxin
first would have left them on 1, which is why the repeats run after each owner's
current-round effects. The gate carries 1092369 and 1092294 in full and prefixes of
963039, 926226, 1090531, 1091585 and 1091904; 1073107 cannot enter because its first round
selects a clan-gated Equalizer Life ability, and 1059149's rounds sit behind the deferred
GhosTown night bonus.

Semantic revision 29 admits the plain `+N Pillz` Victory grammar (`337`, `455`, `503`,
`1054`, `1150`, `1229`, `2262`, `2525`, `4855`, `5258`): the round winner's own Pillz rise
by the printed amount after the bet has been paid. It is admitted the way Victory Life is -
exact printed text and the complete structured shape over every same-text registry record,
printed numbers as authority, the catalog id a structural alias of the definition its text
resolves to - but from the Ability slot only, since no clan prints it as a bonus. The
prediction in "Choosing the next slice" held: the post-round channel already moved Pillz for
Defeat recovery, Victory-or-Defeat, Argos and Komboka, so the engine gained one winner-only
`GainPillzOnVictory` arm beside Victory Life and no other line changed. Like every ordinary
own gain it pays a living owner - the TypeScript guard for a player at zero, which an earlier
owner's repeating Toxin can produce - and a knockout of the opponent changes nothing. The
near-miss boundary is Victory Life's: `+N Pillz` text over a wrong slot or structure, or the
complete shape under other text, rejects when selected, while the prefixed forms (`Stop:`,
`Growth:`, `Degrowth:`, `Confidence:`, `Courage:`, `Brawl:`, `Killshot:`, `Perfect:`,
`Equalizer:`, `Defeat:` and Victory-or-Defeat), `Support: + 1 Pillz` and the capped `+3 Pillz
Max. 9` differ structurally and keep their visible-but-disabled records. The same-text
`+1 Pillz` record over the Victory-or-Defeat shape, which the VOD test had used as an inert
no-op, is therefore a selected hazard now.

The server pins the arithmetic from both sides. Archimedes' `+2 Pillz` takes 12 - 7 + 2 to 7
in 1092141/0, where Petra's Stop Opp. Bonus silences the Riots VOD but not the ability;
12 - 5 + 2 to 9 in 1092201/0; 12 - 9 + 2 + 1 to 6 in 1060341/0; 12 - 1 + 2 to 13 in
1093275/0; and pays beside the VOD in 1092578/0, 1092773/0 and 1092840/0. Corvus Cr's +3
reaches 14 in 1092201/1 while Argos' capped Defeat gain lands on the other side, Mercury's
+2 pays in 1090887/0 and Grudj Cr's in 1060510/0 as his Freaks Poison latches. A loss pays
nothing: Archimedes once per side in 1092992/0 and 1092992/1, Zaveli in 1060510/1, Joy in
1089626/0, and Grudj Cr in 1025525/1 where the owner is knocked out at 0 Pillz. 1092066/0 is
the stopped case, where Markus' Roots bonus stops the ability and only the VOD pays. The
gate grows from 219 to 243 rounds, with 1092992 and 1090887 in full and prefixes of the
others. 1092454, the other draw the family had measured, stops at two rounds because round
2 selects a dynamic `Copy: Opp. Ability`, which replay keeps fail-closed; the round where
Archimedes pays into a knockout (1092454/3: 7 - 5 for a Fury bet of two, + 2, + 1 = 5) is
pinned by the engine test instead.

Semantic revision 30 admits the opposing half, the plain `-N Opp Pillz. Min M` Victory
grammar (`334`, `339`, `343`, `360`, `570`, `854`, `3541`, `5532`, `5682`): the winner takes
N from the opposing player's remaining Pillz, never below M. Admission is the same rule -
exact text, complete structured shape, Ability slot only, catalog id a structural alias of
the definition its text resolves to - and the engine arm mirrors the unconditional Victory
opponent-Life reduction on the other resource: it reads the target after both bets have been
paid, which is where the TypeScript reference applies END modifiers, and a target already at
or below Min is left alone rather than pulled up to it. The near-miss boundary rejects the
text over a wrong slot or structure and the shape under other text; `Stop:`, `Growth:`,
`Brawl:`, `Bet > N Pillz:`, `Defeat:` and clan-gated forms differ structurally and stay
visible-but-disabled, and the dotted `Opp. Pillz And Life` compounds are other grammars.

The corpus pins the floor twice and the trigger four times. Dalhia Cr's `-3 Opp Pillz. Min 4`
takes Callie from 12 - 5 to exactly 4 in 1131294/0 (a rule-3 draw), and in 1091644/1 meets
AI-Lycs already on 4 after his Defeat recovery (6 - 6, recover 4) and changes nothing - the
order of the two owners' effects does not matter there, which is why the round is a valid
pin. A loss takes nothing: Yomi Ld in 924413/0, Gil Cr in 956608/0 (a rule-2 draw),
Baldovino in 1087712/0, Hawkins Cr in 1131294/1, Thorpah Cr in 1023396/0 and Andsom in
946288/2 while his target is knocked out at 0 Pillz. The gate grows from 243 to 260 rounds,
with 924413, 956608 and 1131294 in full, prefixes of 1091644 and 1087712, and 946288
extended from one round to three. 1066481, the one paying round above Min in the corpus
(`5682` takes Lothar from 7 to 6), sits behind a clan-gated `After` bonus in its first
round and cannot enter; the engine test pins reductions above, onto and across the floor.

Semantic revision 31 closes the Pillz family with `+1 Pillz Per Damage` (`809`, `1051`,
`1090`) and its `Symmetry:` form (`1852`): the winner's own Pillz rise by the final resolved
Damage its card dealt. The magnitude is the one Anita's Courage conversion already binds -
the TypeScript multiplier is `card.damage.final`, so Fury and every combat Damage modifier
count - and the Symmetry condition is the predicate the post-round plan already evaluated
for Doela Noel's reduction, so the engine gained one winner-only arm and the plan validator
one rule: Ability slot only, predicate `Always` or `SelectedHandSlotsMatch`. Admission is by
exact text and complete structured shape over the `convert_dmg_to_pillz` special action,
which no other registry record carries; a record that converts Damage to Pillz, prints the
text, or carries the shape under another hand-slot prefix rejects when selected.

Every part is server-pinned. Spade's plain form pays 12 - 10 + 5 = 7 in 1024592/0 and 12 -
11 + 5 = 6 in 1024732/0, both Fury bets, which is also what turns 1024592 from preparation
evidence into a two-round gate member with Arnie's Fury-inclusive Defeat recovery behind it.
Ramak's Symmetry form pays 12 - 5 + 4 = 11 in 1023274/1 and 12 - 3 + 4 = 13 in 1058151/0,
slot 3 against slot 3 both times, and pays nothing when the slots differ: 1011183/3, where
Impudicus' Stop Opp. Ability bonus would have stopped it anyway, and 1023396/1 and
1025102/1, which the gate already carried with the source disabled. A loss pays nothing:
Grace in 1089933/0, Sah Brinak Cr in 1023396/1, Ramak in 1011643/1 and 1025031/2. The gate
grows from 260 to 273 rounds with 1011183 in full and prefixes of 1023274, 1024592, 1024732,
1058151 and 1089933; the two `809`/`1852` disabled records leave the gate's disabled set.

Semantic revision 32 takes two families in one slice because they were the same shape as
the two before. `+N Life Per Damage` (`141`, `189`, `226`, `492`, `1125`, `1224`, `4500`) is
Pillz per Damage on the other resource: the winner's own Life rises by N per point of final
resolved Damage, Ability slot only, and its `Revenge:` (`1661`) and `Confidence:` (`1810`,
printed `+1 Life Per Dmg.`) forms carry the previous-round predicate. Anita's `Courage:`
record stays identity-locked because its condition sits in the position field, which this
grammar requires neutral, so her level-two alias `843` is still refused; the capped `Max.`
forms, the Victory-or-Defeat forms and the `Versus` clan form stay closed. The second family
widens the four plain permanent grammars to carry one predicate: the classifiers now return
the condition their prefix names - `Symmetry:`/`Asymmetry:` from the hand-slot field,
`Revenge:`/`Confidence:` from the previous-round field - and the plan validator admits
exactly those five predicates for a permanent. Nothing changed in the latch itself: the
predicate is judged where every plan predicate is, in the latching round, and the latched
effect carries none. That admits `Symmetry: Toxin 3, Min 0` (`5092`), `Asymmetry: Heal 1
Max. 16` (`5692`), `Asymmetry: Regen 1, Max. 17` (`5693`) and `Revenge: Poison 2, Min 0`
(`3301`); `Defeat`, `Killshot`, `Perfect`, `Backlash`, `Growth`, `Unison`, Victory-or-Defeat
and clan-gated permanents have other structured conditions and stay closed, and a record
carrying two conditions at once matches no grammar.

The candidate lines read 3 and 1, together 4, and the slice unlocked exactly those four
draws. Server rounds pin the plain conversion: Nyema's `492` pays her final 3 Damage beside
the Jungo Victory Life in 1089121/2 (9 + 3 + 2 = 14), Jautya's `4500` takes 7 to 10 in
1092141/1, which had kept that draw to a one-round prefix, and Kenny Cr's `+2` form pays
nothing on a loss in 1093399/2. The gate grows from 273 to 282 rounds. The Revenge and
Confidence forms and the four predicate-carrying permanents have no reachable server round -
877476, 1025279, 924853 and 1130454 all open on an unadmitted source - so their conditions
are pinned by the engine tests: Revenge pays only in a round after a loss, a Symmetry Toxin
latches only under matching slots and pays at once, a Revenge Poison latches only after a
loss and waits a round like every Poison.

What the widening changed in the hazard rule is worth stating. `Growth:`, `Unison :` and
`Revenge:` Poison carry exactly the plain Poison structure - the registry keeps their
conditions in the description alone - so with the plain grammar admitted they are the
complete shape under other text and now reject when selected, where before they were
visible-but-disabled. Every other prefixed permanent differs in a structured field
(`Defeat` in the round requirement, `Killshot` and `Perfect` in their own requirements,
`Symmetry`/`Asymmetry` in the index requirement, `Backlash` in the side, the clan-gated
forms in the clan list) and keeps its disabled record; so do Dope, Consume, Repair, Combust
and Mindwipe, which are Pillz or compound permanents and not this family. The
Symmetry/Asymmetry/Revenge forms would ride the predicates the projection already resolves
and the coverage line for them reads 1; they are admission-only work when a draw needs them.

Semantic revision 33 generalises the post-round opponent-Life reduction. Nothing new
executes: `ReduceOpponentLifeOnVictory` and `ReduceOpponentLifeOnVictoryOrDefeat` were
already engine arms, and the `Defeat:` sibling was already a grammar. What changes is
admission. The Victory form had been a list of reviewed identities, so thirteen printed
abilities with the same structured record were refused for having the wrong id; it is now
read the way `Defeat: -N Opp. Life, Min M` always was - exact printed text, complete neutral
structured shape, card abilities only, and a catalog id that is a structural alias of the
definition its text resolves to. The Victory-or-Defeat form follows on the channel that pays
whatever the outcome. Two members stay identity-locked because the Ability slot is not their
only home: the active Berzerk `bonus:680`, and `1628`, the one member the corpus has also
seen in a Bonus slot.

The printed numbers are the whole boundary. `-{N} Opp. Life Min {M}` and `Victory Or Defeat:
- {N} Opp. Life Min {M}` are rebuilt from the record's own `value` and `valueMin` and
compared to the text, so a record whose text disagrees with either number is refused rather
than trusted in one direction. The conditional forms keep their identity list because their
printed text differs record by record (`Symmetry: - 4 Opp. Life Min 0` spaces the sign,
`Confidence: -3 Opp. Life, Min 0` adds a comma), and Courage `4533` and Growth `1730` stay
deferred: `4533` has no selected observation in the corpus and `1730` is a round-scaled
magnitude rather than a predicate. Both are now refused in a compact plan as well, so a
caller cannot relabel them as the plain grammar.

The server pins both channels. Glenn's `512` takes Uuber's owner from 12 - 6 Damage to
exactly its Min of 3 in 962243/0, where Uuber's own `1628` pays 12 to 11 on the losing side;
Kazayan's `769` pays into a knockout in 962243/2. Mou's `1399`, admitted before and now an
ordinary grammar member, still clamps at its Min of 5 in 926367/1 (8 - 2 Damage = 6, then
- 5 holds at 5). Regan's `1726` pays after *losing* 963039/2: the target is on 3, the
reduction takes it to 1, and the Toxin its owner latched in 963039/0 then takes it to 0.
That round pins the ordering as well as the reduction - a latch pays after its owner's own
current-round effects, and the other order would have left the target on 1. A loss pays
nothing: Dao Wang's `935` and Zinfrid's `594` in 926367, Surstorming's `4948` in 1092840/0
and Dregn Cr's `602` in 1131010/0. A stopped source pays nothing either: Mavi's Stop Opp.
Ability silences Glenn's `512` in 876712/0 and only his 6 Damage lands. The gate grows from
282 to 289 rounds, with 962243 and 963039 extended and 1131010 and 876712 added as prefixes.

One record changes side of the hazard line. `Night: -2 Opp. Life Min 0` (`4750`) carries the
complete Victory shape under text the grammar does not name, so it is now the complete shape
under other text and rejects when selected, where before it was visible-but-disabled - the
same movement `Growth:`, `Unison :` and `Revenge:` Poison made in revision 32. Every other
neighbour differs in a structured field and keeps its disabled record: `Defeat:` in the round
requirement, `Killshot:` in its own, `Victory Or Defeat:` own-Life gains in the side affected,
and the clan-gated `5392`. The three same-text catalog ids that carry no registry definition
at all - Rakhan `978`, Milovan `498`, Fraser `1289` - stay fail-closed, because the alias rule
is catalog authority and description equality never transfers an effect to another card.

Semantic revision 34 takes two same-shape families in one slice again, and neither needed a
new effect channel. `+N Life Per Damage Max. M` (`1146`, `1161`) is the conversion revision
32 admitted under a ceiling: the winner's own Life still rises by N per point of final
resolved Damage, but never past M, and an owner already at or above M gains nothing. The
bound is the one `Heal N Max. M` has modelled on the latch since revision 27, read at the
moment the effect pays, so the engine arm gained a clamp and nothing else. The
predicate-carrying fixed Victory Life family (`2638` `Asymmetry: +3 Life`, `814`/`2113`/`3546`
`Confidence : +N Life`) is the change revision 32 made for permanents, applied to the fixed
Victory Life plan: the classifier now returns the condition its prefix names - the hand-slot
field for `Asymmetry:`, the previous-round field for `Confidence :` - and the plan validator
admits those two predicates beside `Always`. Nothing else moved; the plain grammar is still
generic over Ability and Bonus, while the two prefixed forms are card abilities only,
because no clan bonus prints either.

The two boundaries stay narrow on purpose. A cap and a previous-round prefix have never been
printed on one record, so a `Revenge:`/`Confidence:` conversion with a non-zero `valueMax`
has no reviewed text and rejects in both the compiler and the compact plan rather than being
given a guessed one. Victory Life's structural half now names exactly three condition slots -
none, the `Confidence :` previous-round one, the `Asymmetry:` hand-slot one - so a `Revenge:`
Life, a Courage position or a clan gate keeps its visible-but-disabled record instead of
becoming a near-miss hazard. The corpus has no other record in either new shape, so no source
changed disposition beyond the six admitted here; `2638` simply leaves the gate's disabled
set.

The candidate lines read 2 and 2, together 4, and the slice unlocked exactly those four draws
(1010898, 1011183, 1130609, 1131010), taking strict eligibility from 65 to 69. The server
pins the cap twice, both times from below it: C Dusk's `1146` converts a Fury-inclusive 6
Damage and its owner stops at 8 rather than 11 in 1130609/3 (5 + 6), and converts a plain 4
to stop at 8 rather than 11 in 1131010/2 (7 + 4). Impudicus' `2638` pays its 3 in 1010898/0,
where his Roots `Stop Opp. Ability` bonus leaves Aneta's Courage inert and the two selected
slots differ, and pays nothing in 1011183/3, where the slots differ but he loses. The gate
grows from 289 to 299 rounds: 1010898 and 1130609 join in full, 1131010 runs in full instead
of two rounds, and 1011183 already ran in full. La Salerosa's `1161` and every `Confidence :`
form have no reachable server round - 877167 opens on `Victory Or Defeat : +3 Players Life`
and all four Barcius draws carry the Cosmohnuts `Tune Out` bonus - so their arithmetic is
pinned by the engine tests instead: a capped conversion clamping, paying nothing at the cap
and nothing above it, and Confidence waiting for a round its owner won.

Semantic revision 35 admits `Xantiax: -N Life, Min. M` (`1379` at level 3, `5198` at level
2), the one grammar in the projection that names no outcome and no beneficiary: both players
lose N at the end of the round, neither below M, whoever won. `Xantiax` is flavour on the
printed text rather than a condition - the structured record asks for no current round, no
previous round, no position and no hand slot, and sets `sideAffected` to `both`, which is a
value no admitted grammar had used before. That field is what makes the boundary safe:
every neighbouring Life reduction names `win`, `lose` or a single side, so none of them can
reach this grammar by text alone, and the same-text record under any other structure stays a
selected hazard. Admission is otherwise the ordinary rule - exact text, complete structured
shape, Ability slot only, catalog id a structural alias of the definition its text resolves
to - and the two printed levels are separate records admitted on their own, not aliases of
each other.

The engine arm is the first post-round effect that reads neither the winner nor the owner.
It charges `owner` and `owner.other()` in turn, skipping a side already at or below Min so
the clamp can never revive a player from zero or pull one up to the floor. Nothing else
changed: the plan carries `Always`, because there is no condition to carry.

The candidate line read 3 and the slice unlocked exactly those three draws (1058151, 1059648,
1080464), taking strict eligibility from 70 to 73. All three are server-pinned, and between
them they cover every arm. Xantiax Robb Cr wins in 1080464/2 and is still charged: 6 - 3 = 3
on his own side, while the loser goes 17 - 1 damage - 3 - 2 (the Berzerk `680` behind it) to
11. He loses in 1059648/1 and both sides pay anyway: 12 - 3 damage - 3 = 6 for him, and
11 - 2 (Cyb Lhia's latched Poison) + 3 (Anita's Courage conversion) - 3 = 9 for the winner.
And in 1058151/3 he is knocked out by the round's 7 Damage and the opposing player is charged
regardless, 5 to 2, with his own side floored at zero - which is the Min 0 clamp acting on
both sides at once. The gate grows from 299 to 310 rounds with all three draws in full;
`582`, `859`, `4571`, `4588` and `5881` are already-admitted grammars that those draws
exercise for the first time. The engine test pins what the corpus cannot reach: a reduction
landing exactly on the floor, and a side already there being left alone.

Semantic revision 36 puts the `Confidence:` previous-round predicate on the plain `+N Pillz`
Victory grammar (`1702` `Confidence: +4 Pillz`, `4449` `Confidence: +2 Pillz`). It is the
change revision 34 made to fixed Victory Life, applied to the other resource: the classifier
now returns the condition its prefix names instead of assuming `Always`, and the plan
validator admits `OwnerWonPreviousRound` beside it. No engine work at all - the winner-only
arm and the predicate machinery both already existed, which is what revision 33 looked like
too. The plain grammar is untouched and still unconditional.

One detail keeps the two prefixed grammars apart and is worth writing down, because the
registry would otherwise let them borrow each other's text: Victory Life prints a spaced
`Confidence : +4 Life` and Victory Pillz prints a tight `Confidence: +4 Pillz`. Admission is
by exact text against a `format!` of the structured value, so neither can match the other's
record, and both remain card abilities only. The near-miss boundary grew to match - a
`Confidence: +N Pillz` over a wrong structure is now a selected hazard rather than an inert
disabled source - while `Killshot:`, `Revenge:`, `Stop:`, `Growth:`, `Brawl:`, `Courage:`,
`Perfect:`, `Equalizer:` and `Defeat:` Pillz all differ in a structured field and keep their
visible-but-disabled records.

The candidate line read 2 and the slice unlocked exactly those two draws (924615, 925087),
taking strict eligibility from 74 to 76 of 361. The corpus is thin here and the gate shows
it: of the four rounds in which either record is selected, only one can be reached. Balixto's
`1702` pays in 924615/2 - his side won round 1, he wins round 2 on a bet of 5, and
7 - 5 + 4 = 6 - and that draw joins the gate for three rounds, taking it from 310 to 313. It
stops at three because its capture could not attribute the closing `battles.result` to a
side, so the final round's life is a stale pre-damage snapshot rather than a server fact;
that is also why 924615 is one of the 42 TypeScript replay mismatches, and it is a capture
artifact rather than an engine disagreement. The other three rounds are all unreachable:
1092515/2 (a loss, where the Vortex `577` recovery pays instead) mismatches in its own round
0, 1073010/1 (his side did win the round before, but Spidee's Reprisal `Stop Opp. Ability`
silences him, which is the one round in the corpus that separates the predicate from the
source being live) opens on a deferred `Brawl:` source, and 925781/1 (Monkovski's `4449`, a
loss with no prior win either) sits behind the Cosmohnuts `Tune Out` bonus. The engine test
pins the three negative arms instead: a first round with no previous round to have won, a
won round behind a lost one, and a lost current round behind a won previous one.

One paying server round is thinner evidence than this project usually spends, and it is
admitted here only because the two pieces being composed are each independently pinned - the
plain `+N Pillz` grammar by revision 29's ten paying rounds, and `OwnerWonPreviousRound` by
the predicate machinery every `Confidence:` source already uses. Revision 34 admitted the
same predicate on Victory Life with no reachable server round at all.

### Adding a post-round grammar

The shared parts were factored out on 2026-09-20, so a grammar is now the handful of things
that are actually specific to it. In order:

1. **`combat_stat_compiler.rs`** - a `classify_*` returning the magnitudes and predicate, and
   a `*_shape_matches` that is `POST_ROUND_SHAPE` with the fields the grammar differs in
   overridden. Do not restate the neutral fields: `shape_matches` requires the clan gates,
   the bet link, `valueCondition` and every magnitude flag neutral for all grammars, which is
   what keeps the projection fail-closed as the table grows. State condition slots as
   `(previous round, hand slot)` pairs, never as two independent lists. Add a `has_*_shape`
   so replay preparation can call a complete shape under malformed text a hazard.
2. **`combat_stat_diagnostic.rs`** - the effect in both enums, an arm in
   `shared_post_round_effect`, and the predicate rule in the plan validator.
3. **`engine/mod.rs`** - the `PostRoundEffect` variant and the arm that pays it. Often there
   is nothing to do here: the cheapest slices are the ones where the projection already
   executes the effect on another channel and only admission closes.
4. **`catalog_match.rs`** - a dispatch arm calling `require_catalog_alias`, and a preparer
   that is one call to `prepare_post_round_source` with a closure mapping the classifier's
   output to the two effect representations and the predicate.
5. **`replay/combat_stat_diagnostic.rs`** - a `classify_*` arm returning
   `executes_post_round(...)`, and an `unadmitted_*` clause so near misses stay selected
   hazards instead of becoming inert disabled sources.
6. **Tests and pins** - a compiler boundary test, an engine arithmetic test for the arms the
   corpus cannot reach, and the gate rounds.
7. **Regenerate the derived pins** - the executed and disabled source sets, the eligible-draw
   set, the gate's round and absent counts, the compiler revision and the TypeScript
   provenance fingerprints are not edited by hand. Run

   ```bash
   deno task pins:update
   git diff rust/tests/expect tests/expect
   ```

   and read that diff: it names the ids and draws the slice added, which is the slice's
   unlock evidence and belongs in the commit message. An unlock you did not predict is a
   finding, not a formality - `UR_UPDATE_EXPECT=1` will happily record a regression as
   cheerfully as a win, so the diff is the review, and nothing else is.

Before the refactor a grammar cost about 700 lines across 11 to 14 files, most of it copied;
the shared plumbing is now written once, and what remains is the part that says what the
grammar is.

### Choosing the next slice

Reach and unlock rank differently, and only unlock is worth acting on. Strict construction
rejects a draw at its first unsupported source, so a raw error tally counts sources that
merely co-occur with other blockers. `rust/tests/strict_coverage_blockers.rs` retires each
blocking slot with a neutral filler and retries, collecting every blocker in a draw, then
reports both numbers. It is ignored by default because it is a report rather than a gate:

```bash
cargo test --manifest-path rust/Cargo.toml --locked \
    --test strict_coverage_blockers -- --ignored --nocapture
```

An id named in a candidate family must be a real registry definition, or the family silently
under-reports: an id no definition owns can never appear as a blocker. The list carried
`990` for conditional Copy until 2026-09-17, so that family was only ever scored by `958`.

On 2026-09-20 at revision 36 it scanned 361 complete draws: 76 eligible and 10 refused
structurally, by a Leader or a duplicate character rather than by a missing effect. The
report also prints the blocker sets themselves, smallest first, which is what a family
proposal should be built from: a group is worth proposing only when it covers one of those
sets whole, and anything else merely co-occurs with a blocker that is still there. Revision
26 was chosen from that listing - `3526` alone blocked four draws - and unlocked exactly
those four; revision 27 added the plain Heal grammar as a candidate-family line, read 2, and
unlocked 2; revision 28 measured plain Toxin at 1 and plain Poison at 3 before admitting
them and unlocked 4; revision 29 measured own fixed `+N Pillz` at 2 and unlocked 2,
revision 30 did the same for opposing Pillz reduction, revision 31 measured Pillz per
Damage at 4 and unlocked 4, revision 32 measured uncapped Life per Damage at 3 and the
predicate-carrying permanents at 1, together 4, and unlocked 4, revision 33 measured the
plain Victory `-N Opp. Life, Min M` grammar at 2 and its Victory-or-Defeat form at 2,
together 4, and unlocked 4, and revision 34 measured the capped Life-per-Damage forms at 2
and predicate-carrying fixed Victory Life at 2, together 4, and unlocked 4, revision 35
measured Xantiax at 3 and unlocked 3, and revision 36 measured `Confidence: +N Pillz` at 2
and unlocked 2. Those lines now read 0, which is how a landed family is meant to look.

An id named in a candidate family must also cover the family's other printed levels, or the
line under-reports the same way a missing definition does. Xantiax was scored at 3 by `1379`
alone and read the same with level 2's `5198` beside it, `Killshot: +N Pillz` read 2 as `[2250]` and still 2 as `[2250, 4645]`,
and `Confidence: +N Pillz` read 2 as `[1702]` and still 2 as `[1702, 4449]` - which is only
knowable by adding the line. A second member that changes no count still belongs in it, and
in the slice: `4449` landed with `1702` because it is the same grammar, not because it paid
for itself.

The measurements that chose the last two slices are worth keeping as a record of how the
counts behave. Revision 24's two Copy families unlocked 3 and 2 and together 5; revision
25's two unlocked 3 and 4 and together 7. Neither pair shared a draw, unlike the revision-22
pair that shared six - so neither additivity nor overlap can be assumed, and the split has
to be measured each time.

The Pillz family is done, and so are the Life conversions capped and uncapped, the
predicate-carrying permanents, the opponent-Life reduction on all three outcome channels,
fixed Victory Life with or without its two printed conditions, the both-sides Xantiax
reduction, and `+N Pillz` under `Confidence:`. Before revision 29 the three
Pillz lines together unlocked 8 draws and the three slices unlocked 2, 2 and 4 in turn;
revision 32 then took Life per Damage (3) and the prefixed permanents (1) together for 4,
and revision 33 took the Victory reduction (2) and its Victory-or-Defeat form (2) together
for 4. Both pairs were exactly additive, unlike the revision-22 pair.

What the scan reads at revision 36 has no three-draw source left in it - Xantiax was the
last one - and eight two-draw single-source sets, unchanged in count by this slice: the
wider permanent-Life line (the losing-round `4561` and Growth `1282` forms, the only family
among them), and the lone sources `304` `Courage: -4 Opp. Dmg, Min 2`, `490` `Confidence:
Stop Opp. Ability`, `912` `Defeat: -2 Opp. Pillz, Min 4`, `1474` `Stop: Damage +4`, `1488`
`Brawl: Power And Damage + 1`, `2250`/`4645` `Killshot: +N Pillz`, the clan-gated `5113`
`+1 Dam./ Life Lost Max. 6` and `5681` `After [clan:27][clan:29]: -2 Opp. Pow. & Dam.,
Min 2`. Nothing on that list is cheap in the way the last four slices were, because each of
them wants a channel or a context the projection does not have: a Courage position on a
combat stat, a conditional Stop, a losing-side Pillz reduction, a Stop-triggered increase, a
Brawl round counter, a clan gate. The board of admission-only slices is empty.

`Killshot: +N Pillz` is the one that looks cheapest on count and should not be taken on it.
Its
`sureshot` current-round channel exists nowhere in the projection, and the corpus cannot
justify building one: of 23 selected Killshot rounds across every Killshot grammar, exactly
one actually triggers (877023/1, Valentina Ld's `-3 Opp. Life Min 0` at 72 attack against 7),
and the `Killshot: +N Pillz` grammar has **no** paying observation at all - Radamir loses in
949959/0 and Barcelo is never selected in either draw that needs him. The 15 non-firing
rounds pin only the negative half, that a plain victory does not pay. Play a Killshot Pillz
card into a doubled attack before coding this one, or take it together with the other
Killshot grammars once the corpus has more than one firing round in total.

`rust/tests/killshot_evidence_report.rs` answers that question in one command, so a newly
captured battle does not have to be read by hand. Over the 355 replayable draws it finds 22
selected Killshot rounds across 18 definitions and exactly one that fires - 877023/1,
Valentina Ld's `4459` at 72 attack against 7 - and lists the near misses, the closest being
875272/3, where Baresco reached 28 of the 30 it needed. Re-run it after any new capture:
a second firing round, on any Killshot grammar, is what turns this family from a guess into
a slice.

Measure again before choosing; the counts have moved after every slice.

On 2026-09-20 the corpus was re-read for evidence rather than for counts, and two of the
sources this section had written off as two-draw singletons are both measured and pinned:

* `304`/`961` `Courage: -4 Opp. Dmg, Min 2` unlocks 2. Battle 1078999 round 2 pins the
  arithmetic and exercises the floor in the same round: Hattori moves first and loses, and
  Lothar's printed 5 Damage resolves as 2, which is `max(5 - 4, 2)` and not the unfloored
  1. The life ledger agrees independently - side 1 goes 7 to 5. The same round also shows
  Lothar's own `-3 Opp Power, Min 4` taking Hattori's 8 Power to 5, so both sides' effects
  are visible and separable.
* `912` `Defeat: -2 Opp. Pillz, Min 4` unlocks 3, not the 2 this section claimed. Battles
  1092515 round 0 and 1092201 round 2 both pin it away from the floor once the opponent's
  own bet and their independent `Victory Or Defeat: +1 Pillz` bonus are accounted for:
  12 - 5 + 1 - 2 = 6 and 11 - 0 + 1 - 2 = 10, both matching the server exactly, and neither
  matching the no-effect reading. Battle 1088480 round 2 is a floor case that distinguishes
  nothing, which is worth knowing but is not the evidence. The clan-gated sibling `4673`
  is a different grammar and is not in the family.

Together they measure 5 and are exactly additive, which makes them the largest slice
available at revision 36, and revision 37 took both: eligibility went 76 to 81 and the gate
310 to 318.

The Courage half turned out to be admission-only after all, which this section had got
wrong. `classify_position_numeric` already put `OwnerMovesFirst` on a fixed numeric effect,
and `numeric_effect` already covered an opponent-Damage decrease; the only thing rejecting
`304` was `numeric_description_body_matches`, which knew `-4 Opp Damage, Min 2` and
`-4 Opp. Damage, Min 2` but not the abbreviated `-4 Opp. Dmg, Min 2` Hattori prints. One
alternative spelling on that arm unlocked both draws. The lesson is worth keeping: a source
the report lists as needing "a channel the projection does not have" may only need its
printed spelling, and the cheapest way to tell is to read the classifier chain rather than
this section's prediction of it.

The Pillz half was a real grammar: `classify_defeat_opponent_pillz` with the neutral
post-round shape on `CurrentRoundRequirementV1::Lose`, one effect in each enum, one engine
arm and the usual preparer and hazard clause. It composes two already-pinned pieces - the
Victory reduction's post-bet arithmetic and Min clamp, and the Defeat channel's trigger,
which the reviewed opponent-Life sibling established pays out even from a knocked-out owner.
The corpus has no knocked-out owner carrying this ability, so that arm rests on the sibling
and is pinned by an engine test rather than by a capture.

`490` `Confidence: Stop Opp. Ability` was read the same way and is **not** ready. It is
selected exactly once in the corpus (1091644 round 2) with Confidence genuinely satisfied,
but the opposing ability it would have stopped is itself `Stop Opp. Ability`, which carries
no numeric payload - so the round resolves identically whether the stop fired or not. A
precondition-satisfied selection that proves nothing is still negative-only.

Pillz is a resource the post-round channel already moved - Defeat recovery, Victory-or-
Defeat Pillz, Argos and Komboka all wrote it - so each of the three slices was one engine
arm and one compiler grammar, exactly as this section had predicted for own fixed `+N
Pillz`: the opposing reduction reused the Min clamp the Life reductions modelled, and the
conversion reused Anita's final-damage magnitude and Doela Noel's Symmetry predicate.
Revisions 32, 33 and 34 show that same-shape families can be taken in one slice when each is
measured as its own line first and the combined line is measured too: Life per Damage and the
prefixed permanents were 3 and 1 alone and 4 together, the two opponent-Life channels were 2
and 2 alone and 4 together, and the capped conversion and predicate-carrying Victory Life
were 2 and 2 alone and 4 together, so nothing was hidden by overlap any of the three times.
Revision 33 cost no engine work at all and revision 34 cost one clamp on an arm that already
existed, which is what a slice looks like when the family it widens is one the projection has
already executed.

The two families revision 22 took were cheaper than this section predicted. It claimed a
post-round plan carries no predicate and that adding one was the shared change both needed;
in fact `active_effect` already evaluated `predicate_matches` over the post-round effect, so
only admission was closed. Check the code before pricing a slice from this paragraph, and
rerun the measurement after any admission change rather than trusting the numbers above.

Semantic revision 38 takes the Killshot opponent-Life reduction (`1204`, `1670`, `1779`,
`1959`, `4459`, `4785`, `5461`, `5530`): the owner's final attack being at least double the
opposing one reduces the opposing player's Life by N, never below M. It measured 2 and
unlocked exactly 2 - `874837` and `875272` - taking eligibility from 81 to 83. Note that
`875272` is the same draw this section named as the corpus's closest Killshot near miss; it
became eligible because its Killshot *Life* blocker cleared, not because the near miss was
resolved.

Two things this section had wrong are worth recording, because both were found by
measurement rather than by reading.

* **The `sureshot` channel is worth 6 draws, not 15 sources.** "Fifteen blocked definitions
  are waiting on it" is a reach number and does not convert. Measured as family lines: the
  opponent-Life form 2, own Pillz 2, own Life gain 1, `+N Pillz And Life` 0, and *every*
  Killshot grammar together 6 - mildly super-additive, since 2+2+1 is 5. There are also 24
  Killshot definitions in the registry, not 15.
* **The `Killshot: +N Pillz` line under-reported.** It carried `[2250, 4645]` and is missing
  the third printed level `4311`. It still reads 2 with it, which is luck rather than
  correctness, and is exactly the failure this section warns about two paragraphs above.

The price was one new arm, not a new channel, and this section's "exists nowhere in the
projection" was misleading about cost rather than wrong about fact. The registry has always
parsed `sureshot` (`effect_registry.rs`); `PostRoundShapeV1` already varied
`current_round`; the Min-clamped opponent-Life reduction was already executed on three
channels; and both final attacks were already in scope in the `commit` match arm where the
Victory and Defeat guards live. Only the guard expression was new. Read the classifier chain
before believing a price in this section - that is now four times in two days.

The guard's spelling is the one subtlety. `Condition::Killshot` in the reference is
`attack >= opp_attack * 2` with **no** win requirement, unlike its `Backlash` neighbour, so
it must not be written as `owner == winner && ratio`: at equal attacks, which zero power
reaches, the ratio holds while `round_winner` may hand the round to the other side. The
corpus cannot reach that round, nor the exact-double boundary, nor the Min clamp, so all
three are pinned by an engine test instead. The corpus pins the two halves that matter:
1337321/1 pays (Drakorah Cr at 56 against 14, 12 - 5 damage - 6 to 1) and 1337230/0 does
not (the same card wins at 80 against 49, short of the double).

Two defects on `main` surfaced while landing this and are fixed here rather than separately.
The inventory test `observed_previous_round_inventory_is_exact_and_fail_closed` was failing
before this slice began: `591` `Confidence : -1 Opp. Power, Min 1` arrived with the
2026-09-20 Dojo captures and was in neither of the test's lists. It is admitted, not
deferred - the prefix match already tolerates the space before its colon. The derived pins
were also stale: the same captures took the scanned corpus from 361 draws to 364 and moved
`effectRegistryFingerprintFnv1a64`, which had not been regenerated.

Semantic revision 39 admits the Courage form of the Victory opponent-Life reduction
(`3314`, `4531`, `4532`, `4533`): the owner moving first and winning reduces the opposing
player's Life by N, never below M. It measured 2 and unlocked exactly 2 - `957028` and
`1081879` - taking eligibility from 83 to 85.

The candidate line this came from was wrong in a way worth naming, because it is a new
failure mode for this section. It read `[1730, 4533]` under the heading "conditional Victory
opponent-Life", and those are **two different grammars filed under one name**. `1730` is
`Growth: - 1 Opp. Life Min 4`, a round-scaled magnitude, and it stays deferred. Courage is
four printed levels across two cards, not the one the line carried. Split and corrected, the
Courage line measures 2 where the mixed line measured 1. A line that names a grammar it does
not contain under-reports exactly like a line that omits a printed level, and it is harder
to spot.

Dragomer Cr is the caution on the other side. It prints the reduction at three levels, but
levels 4 and 5 print catalog ability `3001` and `2302`, and **neither has a registry
definition at all** - so they stay fail-closed by simply not being listed, the Doela Noel
level-one `4843` case again. Enumerating a card's levels is not the same as enumerating a
family's definitions, and only the second is what the line needs.

The price was admission-only plus one predicate arm, as the identity table's own comment had
implied was impossible. Courage carries its condition in the `positionRequirement` field,
which `victory_opponent_life_shape_matches` had listed among the predicates it returns
`false` for; the whole grammar change is that arm mapping `OwnerMovesFirst` to
`(Attacker, Any, Any)`. That makes four consecutive candidates this section over-priced.

**The evidence is thin and this is the exception, not the rule.** The corpus has exactly one
paying round: 1091848/1, where Ligea moves first and wins, and side 1 goes from 10 to 6 -
combat damage 1, then the Courage 3, with Min 1 not binding. Two further selections
(1081879/0, 926470/3) are silenced by an opposing `Reprisal: Stop Opp. Ability` and a
`Stop Opp. Ability`, which pins liveness rather than arithmetic, and two more lost the
round. It is admitted on the same basis revision 36 used: every piece it composes is already
pinned separately - the Min-clamped opponent-Life reduction by revision 33's rounds, and
`OwnerMovesFirst` on a post-round plan by Anita's existing arm - so the slice adds a
composition rather than new arithmetic. The round where the owner wins having moved second,
and the Min clamp under this predicate, are pinned by an engine test instead.

### Per-decision admission instead of whole-draw admission

Every slice above widens what the projection understands. There is a second axis, which
widens nothing and costs no ability work: stop asking whether the *draw* is understood and
ask whether *this decision* is. A decision in round `r` only explores the cards still in
hand, and the advisor reconciles life and pillz against the server's snapshot rather than
recomputing the rounds already played, so a decision can be sound in a draw that is not.

`rust/tests/partial_admission_report.rs` measures it by retiring each already-played slot
with a neutral filler and retrying construction, over the 355 replayable draws and the 1,222
decisions in them:

| rule | decisions | share |
| --- | --- | --- |
| whole draw (today) | 260 | 21.3% |
| remaining cards executable | 388 | 31.8% |
| remaining cards, no persistent history | 338 | 27.7% |

The middle row is the optimistic bound - it assumes everything already played has finished
paying - and the last row subtracts every decision where an already-played card prints text
that could still be paying out. The real rule has to be at least as strict as the last row,
because a latched permanent is not history: an unsupported card that latched a Poison keeps
taking Life in every later round, and a projection that forgot it would be quietly wrong
rather than loudly absent.

The gain concentrates late, as it must, because round one is the whole draw by definition:

| round | decisions | remaining | no persistent history |
| --- | --- | --- | --- |
| 1 | 353 | 75 | 75 |
| 2 | 350 | 92 | 87 |
| 3 | 300 | 100 | 88 |
| 4 | 219 | 121 | 88 |

Counted in games rather than decisions it reads better than the decision share suggests:
**76 draws are usable today, and 163 would have at least one usable decision**, because a
draw with one unsupported card usually becomes readable once that card is spent.

What it would cost: a second `CatalogCombatStatProjectionV1` variant that takes the played
slots as input, a compiler answer to "can this unsupported source still be paying?" that is
better than the report's keyword scan (the scan is a measurement heuristic and must not
become a semantic rule), the played-slot set threaded through the JSONL protocol and its
provenance, and a visible marker wherever a partially-informed recommendation is shown. The
fail-closed property has to survive all of it: an unsupported source is still never a no-op,
it is a reason to refuse the decision it can reach.

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

The second vertical slice adds a manual four-round session and exact continuation policy.
`--interactive` retains committed rounds in the real engine, alternates the explicit first
mover, and requests the revealed opposing card before second-mover advice. From round 2,
nonterminal samples recurse to exact win/draw/loss values with the same essential
information-set rule as `Policy.ts`: our response can vary by visible card but not by hidden
pillz or Fury. Cancellation unwinds every made round before returning. Round 1 keeps the
captured-reply-weighted opening estimate; rounds 2–4 use the conservative exact continuation
policy. The opening prior is the literal 198-play `OPENING_REPLY_COUNTS` table from the
TypeScript advisor, captured as of 2026-09-13, with Laplace +1 for unseen wagers. It is
historical provenance, not a table regenerated from the current corpus. The round-two slice
also adds a single-thread blind-second pass in manual sessions: in rounds 2-4 while the
opponent chooses, every unplayed opponent card and hidden wager is a hypothesis and every
row remains one fixed reply. A full hypothesis column is committed transactionally, so a
deadline or policy cancellation never publishes incomparable rows. The visible card then
replaces that provisional ranking with the ordinary precise second-mover search. This does
not add a Rust live-capture client or a Rust worker pool; its JSONL process boundary is used
by the TypeScript-hosted integration below.

The server-backed advisor path loads captures `877636`, `877812`, `925674`, `925719`,
`1024673`, `1060199`, `1061897`, `1069813`, `1081463`, and `1089346` with `--replay`.
Strict catalog eligibility is necessary but not sufficient here: `970972` constructs under
`RequireFullyExecutableDraws` yet is refused by `--replay`, because that night battle records
captured bonus id `38` where the catalog resolves its own id `25` to registry definition
`36`. The advisor keeps that capture/catalog source-identity cross-check rather than
preferring either side.
It derives both exact hands, resources, night state, recording side, and each round's mover
from the normalized capture; rejects any capture/catalog source-identity disagreement; and
requires complete server card evidence. Before every recorded move it renders the same TUI
and grades that move against the current ranking. It then commits the actual pair of moves
and checks power, damage, attack, winner, life, and pillz before advancing. Across these
gates it covers complete matches up to all four rounds and both FIRST and SECOND information
opening heuristic and exact rounds 2–4 policy. It is captured replay, not yet the live
capture stream or full TypeScript opening policy.

Do not revive the old perfect-information recommendation model as the live advisor. Port
the current TypeScript behavior deliberately:

- allocation-free make/unmake search;
- depth-2 work units and cancellation;
- the conservative information-aware policy for hidden pillz and Fury;
- opening-prior refresh from later captures;
- blind-second handling (manual single-thread slice landed);
- visible-percent, knockout, safety, then cost ranking.

Keep the old Rust solver available as a historical reference until equivalence tests cover
the intended replacement.

### 5. TypeScript-hosted Rust worker (V3)

V3 uses a versioned, one-request-per-process JSONL worker. Build it with
`deno task rust:worker`, or build and exercise the real process boundary with
`deno task rust:worker:test`. The TypeScript advisor remains the owner of live capture state,
policy selection, cancellation, and terminal rendering. Rust is off by default;
`deno task advise --rust=compare` keeps TypeScript authoritative while comparing supported
FIRST, SECOND, and blind-second decisions, and `--rust=use` installs only a structurally
validated complete Rust result in the existing TypeScript TUI. A launch, protocol,
provenance, history, information-set, action, completion, or result-validation failure
automatically leaves or returns the decision to TypeScript.

The narrow boundary is intentional: the worker strictly validates canonical-input
fingerprints plus compiler, catalog-context, and advisor-policy semantic revisions; echoes
that exact provenance on every response; replays supplied resolved history; binds SECOND to
the revealed opposing slot,
and checks the legal action matrix and response bounds against TypeScript's corresponding
mode. Compact per-hidden-wager SECOND outcomes are validated against their aggregates and
rebuild the existing opponent-read panel rather than degrading the TUI. `--rust=compare`
reported `rust match` for all five decisions in capture `877636`: opening SECOND, exact
round-two FIRST, round-three blind-second, revealed-card SECOND, and round-four FIRST. This
does not prove semantic equivalence on every request and is neither full replay parity nor
complete engine parity. In-process FFI remains a later consideration only after this
protocol and engine behavior have stayed stable.

The real-process gate runs admissible requests from every rule-10 strict draw: `877636`,
`877812`, `877950`, `925674`, `925719`, `1024673`, `1060199`, `1061897`, `1069813`, and
`1089346`. It pins complete TypeScript/Rust semantic matches for opening and exact FIRST,
opening SECOND, exact SECOND (including hidden-wager outcomes), and exact blind-second
decisions, including at least one match on every one of those ten draws. Captures `1061897`
and `925674` open as SECOND; `1069813` and `1089346` open as FIRST. Each TypeScript
comparison finishes before another `Game` is constructed because the reference still owns a
process-global battle cache. Capture `1081463` remains a standalone Rust replay gate: its
battle-rule id is 3, so the rule-10 TypeScript-hosted worker deliberately rejects it before
launch. Strict draw `970972` is rule 2 and is refused by the Rust replay path itself, so it
enters neither gate. The three rule-6 strict draws are Dojo records: `830285` and `1294430`
replay exactly and `869944` stopped mid-match, but the hosted worker is rule-10 only, so
none of them can enter the bridge whatever their testcase says.

## Performance measurement

Measure engine and solver performance separately:

- Engine: replay identical normalized rounds and report time per resolved round.
- Search: solve identical positions with identical semantics and report nodes, elapsed
  time, peak memory, and result checksum.
- Build Rust with `--release`; warm both runtimes; use multiple alternating samples; keep
  debug output disabled; record machine and commit hashes.

A faster answer from a different policy or a smaller tree is not an implementation speedup.
Correctness and semantic equivalence are gates before headline comparisons.

`deno task time-rust` (`tests/RustCompare.bench.ts`) drives real decision points from
strict-eligible captures through both implementations and prints each side's time beside the
semantic verdict, because a faster answer that differs is not a faster answer. It needs the
release worker, so run `deno task rust:worker` first. Every row below reported `rust match`.
One local run on 2026-09-17, median of three, single-threaded on both sides:

| Decision | Units | TypeScript | Rust (whole process) |
| --- | --- | --- | --- |
| round 1 opening FIRST (`1024673`) | 8464 | 56 ms | 48 ms |
| round 1 opening SECOND (`1061897`) | 2116 | 19 ms | 45 ms |
| round 2 exact FIRST (`877636`) | 3519 | 1866 ms | 106 ms |
| round 3 blind-second (`877636`) | 836 | 17 ms | 43 ms |
| round 3 exact SECOND (`877636`) | 418 | 7 ms | 43 ms |
| round 3 exact FIRST (`1069813`) | 252 | 7 ms | 43 ms |
| round 4 exact FIRST (`877636`) | 11 | 0 ms | 41 ms |

The Rust column is whole-process wall time — spawn, canonical data load, search, response —
which is what the host actually waits for. About 40 ms of it is that fixed cost, so every
row under ~250 units is measuring startup rather than search, and TypeScript wins those
outright by already being warm. The one row where the search dominates is round-two exact
FIRST, at roughly **18x**. That is the shape to expect: the process boundary costs a flat
40 ms and buys back an order of magnitude only once the tree is big enough to pay for it.

### Is an exact opening affordable yet?

Round one is a deliberate model choice in both implementations, not a speed limit either
one hit. `Search.ts` sets `openingEstimate = round === 1` and `search_with_control` picks
`EvaluationKind::OpeningEstimate` below `rounds_played >= 1`; both then score the whole
8464-pairing matrix with the same one-round position heuristic and weight replies by the
same fixed 198-play prior. The two round-one rows above match because they are running the
same model, not because Rust solved anything TypeScript could not.

Forcing `ExactContinuationPolicy` at round zero in a local throwaway build measured the
complete exact opening in release Rust at **6.2 s on the supported demo draw and 11.6 s,
14.2 s and 17.7 s on captures `925719`, `1024673` and `1089346`** — single-threaded, all
8464 units, no deadline cutoff. At the 18x ratio above the same work in TypeScript would be
roughly two to five minutes, which is why the heuristic exists.

So an exact opening is not out of reach in Rust the way it is in TypeScript. It is now a
real mode rather than a measurement; see below.

### The exact opening

`OpeningPolicy::ExactContinuation` solves round one with the same conservative continuation
policy rounds two through four already use, instead of the one-round position heuristic.
Reach it with `deno task rust:advise --exact-opening`, or through the hosted worker with
`deno task advise --rust=use --exact-opening`. It is off by default and has no effect once a
round has been played, because later rounds are exact under either policy.

Measured on this machine, release, single-threaded, complete with no deadline cutoff:

| Information set | Units | Time |
| --- | --- | --- |
| SECOND, opponent's card visible | 2116 | 2.0-5.8 s |
| FIRST | 8464 | 6.2-29.8 s |

SECOND is roughly four times cheaper because the opponent's card is already known, so the
matrix is one card wide rather than four.

Three things had to be separated to make this correct, because the historical pair of
evaluators agreed on all of them and the code had conflated them. How a nonterminal leaf is
scored, how the opponent's current reply is weighted, and whether the Worst column is a
guarantee are independent decisions. An exact opening solves its leaves and earns a real
Worst, but still weights the opponent's reply by the captured 198-play prior, because that
prior is empirical information about what opponents actually open with and says nothing
about how the resulting position should be scored. `EvaluationKind::scores_exactly` and
`weights_by_opening_prior` name the two axes; TypeScript's `Search` gained the same split as
`openingEstimate` and `openingPrior`. Reading `openingEstimate` as "this is round one" was
wrong in three places on the host side and each one was a real defect, caught by the parity
gate rather than by review.

Parity is checkable here, which it would not have been otherwise. A Rust exact opening
compared against the live TypeScript heuristic proves nothing: they answer different
questions, and gate 5 above only admits a comparison when both sides use the same evaluator.
So `Search` has a reference `exactOpening` mode that the live advisor never sets, and
`tests/solver/ExactOpeningParity.test.ts` runs both implementations over the same opening
root and requires `rust match` on every candidate's average, worst, ceiling, displayed
percent, KO and risk shares, and the chosen best move. It is skipped unless `UR_SLOW_PARITY=1`
because TypeScript needs about seventy seconds for the SECOND set that Rust finishes in two.

The hosted bridge carries an `opening_policy` field on every V3 request and the worker echoes
which evaluator actually ran, so a host that predates the field keeps its old behaviour, an
older worker rejects the unknown field outright, and a response that solved an opening nobody
asked to solve is rejected as a mismatch rather than accepted as a bonus. Advisor policy
semantic revision is 2. In `--rust=compare` an exact opening reports `rust exact opening ·
not comparable` rather than `differs`, because there is nothing there to disagree with.

The advice genuinely changes. On capture `925674`'s opening the heuristic recommends Aegis Cr
at five to eight pillz; the exact solve puts Mou at one pillz on top and does not rank Aegis
Cr in the first four at all.

What is still open: the search is single-threaded, so FIRST at 6-30 s is a parallelism
problem rather than an algorithmic one, and there is no unit splitting on the Rust side at
all. Deadline-bounded partial results exist in the protocol but a partially evaluated root
matrix cannot be ranked honestly, so a budget expiry currently falls back rather than
publishing a half-searched opening.

## Working commands

```bash
# TypeScript reference
deno test -A --no-check tests/replay/
deno test -A --no-check

# Rust foundation
cargo fmt --manifest-path rust/Cargo.toml -- --check
cargo check --manifest-path rust/Cargo.toml --locked --all-features
cargo test --manifest-path rust/Cargo.toml --locked

# Head-to-head timing (needs the release worker)
deno task rust:worker
deno task time-rust
```
