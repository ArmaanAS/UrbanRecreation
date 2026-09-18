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
Replay provenance records compiler/policy semantic revision 26 for the current scope.

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
A mismatch rejects the strict draw. Compiler/policy provenance is revision 26.

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
context. Compiler/policy revision 26 records this semantic boundary together with the exact
Equalizer multiplier, the bounded Confidence/Revenge/Frozn slice, the identity-locked
Defeat-recovery post-round plan, the identity-locked Victory-or-Defeat Pillz family with its
both-owner post-round execution semantics, Argos' identity-locked capped Defeat gain, and the
exact-structured positive Victory Life and Victory-or-Defeat Life plans, the identity-locked
Equalizer opponent-Life plan, Anita's identity-locked final-damage Courage conversion, the
two reviewed unconditional Victory opponent-Life reductions, and Lianah Ld's identity-locked
latched Heal.

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
registry schema and source fingerprint, compiler/policy revision 26, and catalog-context
policy revision 3.

The complete 322-game replay-ready corpus supplies a construction oracle: 2,576 card slots
were derived using only catalog clans, explicit night state, and these Oculus rules. There
are 38 Oblivion slots; all 18 description mismatches are captures where the server had
already replaced printed `Copy: Opp. Ability` with the opponent-dependent copied result.
Every one of the other 2,538 slots matches captured bonus presence and description exactly.
On 2026-09-18, the deterministic strict-coverage regression scanned all 359 captured
complete 4+4 hands with canonical `data/data.json`, battle-card overrides, captured
`abilities.json`, and each capture's rule, night, life, and pillz context. It constructs
`CatalogCombatStatMatchV1` under `RequireFullyExecutableDraws`; exactly 43 capture ids
are eligible: `830285`, `869944`, `874520`, `875098`, `875322`, `877636`, `877687`,
`877773`, `877812`, `877860`, `877950`, `878011`, `878056`, `924257`, `925254`, `925674`,
`925719`, `925796`, `943111`, `946112`, `947228`, `949750`, `970972`, `1011712`, `1024673`,
`1058366`, `1059030`, `1059454`, `1060052`, `1060199`, `1061897`, `1065812`, `1069813`,
`1070207`, `1072715`, `1078906`, `1079482`, `1081463`, `1089346`, `1090607`, `1091235`,
`1092909`, and `1130833`.
Revision 21 added `878011`, `925254`, and `1078906` to revision 20's fourteen; revision 22
then added `875098`, `875322`, `1011712`, `1059030`, `1059454`, and `1090607`; revision 23
added `877687`, `924257`, `1070207` and `1091235`, the four the report had predicted for
Protection; revision 24 added `943111`, `946112`, `947228`, `1065812` and `1092909`, the
five predicted for the two Copy families together; revision 25 added `874520`, `925796`,
`949750`, `1058366`, `1060052`, `1072715` and `1130833`, the seven predicted for Attack per
opposing Damage and Defeat opponent-Life; revision 26 added `877773`, `877860`, `878056` and
`1079482`, the four the blocker-set listing had attributed to `3526` alone.
That is six for two families the report had predicted would unlock three each, because six
draws were blocked by *both* families at once — which is exactly why reach and unlock are
measured separately, and why the measurement has to be rerun rather than added up. Catalog
eligibility is a strict whole-draw admission measurement, not proof of full engine or
TypeScript solver parity;
the 189-round immutable diagnostic gate supplies the separately checked sequential replay
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

Two choices are deliberate and unobserved. A latched owner taken to zero is not revived by
a repeat: the repeat is ordinary Life, not Reanimate, and no capture shows a Heal on a KO'd
owner. And admission is Lianah's identity alone, on her level-3 card, in the Ability slot,
with her exact text and shape. The other plain `Heal N Max. M` records - `649`, `751`,
`963`, `1501`, `3118`, `4625`, `5341` - carry the identical structured shape and have
server evidence of their own (`924669/3`, `1059895/3`, `1080877/3` each pay 1), so widening
to the grammar is admission-only work on the same latch; they stay visible-but-disabled
until that is done, while Lianah's id or text under any other shape rejects when selected.
`Defeat : Heal` latches on a loss, `Asymmetry: Heal` on a hand-slot predicate, and Poison,
Toxin and Regen need latch variants of their own (opponent-targeted with a Min, and Toxin
and Regen pay in the latching round), so they are not this slice.

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

On 2026-09-18 at revision 26 it scanned 359 complete draws: 43 eligible and 10 refused
structurally, by a Leader or a duplicate character rather than by a missing effect. The
report also prints the blocker sets themselves, smallest first, which is what a family
proposal should be built from: a group is worth proposing only when it covers one of those
sets whole, and anything else merely co-occurs with a blocker that is still there. Revision
26 was chosen from that listing - `3526` alone blocked four draws - and unlocked exactly
those four.

The measurements that chose the last two slices are worth keeping as a record of how the
counts behave. Revision 24's two Copy families unlocked 3 and 2 and together 5; revision
25's two unlocked 3 and 4 and together 7. Neither pair shared a draw, unlike the revision-22
pair that shared six - so neither additivity nor overlap can be assumed, and the split has
to be measured each time.

What is left still ranks clearly. The rest of permanent Life - Poison, Toxin, Regen and
the other Heals - unlocks 8 draws on its own and 12 together with the Life-per-Damage
conversions, still more than everything else combined; the three Pillz families together
unlock 4, opposing Pillz reduction 2, and everything else 1 or 0. No single source blocks
more than two draws now: the smallest sets are `1474` `Stop: Damage +4` and `5681` `After
[clan:27][clan:29]: -2 Opp. Pow. & Dam., Min 2`, two draws each.

The latch now exists, so the remaining permanent Life is admission plus latch variants
rather than a new mechanism. The cheapest half is the plain `Heal N Max. M` grammar on the
existing `HealLife` latch, which has three independent paying rounds behind it; Poison and
Toxin need an opponent-targeted decreasing variant with a Min, and Toxin and Regen pay in
the latching round where Heal and Poison do not, which the structured records distinguish
only by description and which the TypeScript `delayed` flag encodes. Rerun the measurement
before pricing any of it: the Life-per-Damage overlap and the three Pillz families have not
moved in two revisions, and the two-draw sets are cheap enough to take alongside.

The two families revision 22 took were cheaper than this section predicted. It claimed a
post-round plan carries no predicate and that adding one was the shared change both needed;
in fact `active_effect` already evaluated `predicate_matches` over the post-round effect, so
only admission was closed. Check the code before pricing a slice from this paragraph, and
rerun the measurement after any admission change rather than trusting the numbers above.

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
enters neither gate. The two rule-6 strict draws are Dojo/incomplete records without TypeScript testcases
and cannot enter the hosted bridge.

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
