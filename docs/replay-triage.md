# Replay triage — engine vs server mismatches

Generated from `deno test -A --no-check tests/replay/` on 2026-09-10 against 56 captured
battles (52 replayable: 30 exact, 22 mismatched). Each entry is the first mismatching round
of one battle; the engine value is shown first, the server value second. Battle ids refer
to `captures/games/<id>.json`, which has the full context.

Grouped by suspected root cause, most frequent first. Fixing the top few groups should turn
most of the 22 failures green.

## 1. Day / Night conditions — engine hard-codes `day = true`

`PlayerRound.day` always returns true, so "Night:" abilities never fire and "Day:" always do.
Tournament games are played at whatever the server clock says; the server does not send
day/night in `battles.status`, so we need to infer it (probably from `creationTime`, real
UR alternates on a fixed schedule) or record it.

- 877575 r0 — Figaro "Night: Power And Damage +1": engine 7/4, server 8/5.
- 878120 r0 — Skeletrezar "Night: Stop Opp. Ability" did not stop Buck's Power +2: engine power 7 won, server power 5 lost.

## 2. Support: Attack +N — wrong count

Server counts every same-clan card in the hand (played or not) for the whole game.

- 874520 r0 — Aurora, 4 Rescue, Montana -12 min 8: engine 20, server 14 (7×2 + 12 − 12).
- 875032 r3 — Anita, 4 Rescue, Urbex bonus -10 min 3: engine 15, server 9 (7×1 + 12 − 10).
- 867173 r2 — Sasha "Support: Attack +4" (4 Skeelz = +16) vs AI-Lycs "Equalizer: -3 Opp Attack, Min 5": engine 21, server 17 (7 + 16 − 3×2). Equalizer scales by the *opposing* card's stars (Sasha is 2★).

## 3. Montana bonus "-12 Opp Attack, Min 8" not applied / min clamp wrong

- 875098 r0 — Sue vs Miss Cusaghi ("Cancel Opp. Power Modif."): engine 18, server 8. The bonus was skipped entirely; suspect the ability string fails to parse and the whole card's modifiers are dropped.
- 875272 r2 — Wesley (6×1 + 12 support) vs Don Cr (-12 min 8 bonus, -4 min 2 ability): server Wesley 4, Don 8 wins; engine has Wesley winning.

## 4. Brawl (per opposing card of the same clan) — not implemented

- 874590 r0 — Karkass Cr "Brawl: Damage + 1" vs 4 Rescue: engine 4, server 8.
- 876752 r1 — Macey Rook "Brawl: - 1 Opp. Life Min 0" + bonus "-2 Opp. Life Min 2": server post-round −2 then −3, life 0; engine life 1.

## 5. Growth / Degrowth (scales with round number) — wrong value

- 874642 r0 — Nidory "Degrowth: Power And Damage +1" vs Lothar "-3 Opp Power, Min 4": engine power 8, server 7.
- 874399 r1 — Nidory vs Sue "-1 Opp Power And Damage, Min 3": engine damage 4, server 3.

## 6. Modifier ordering: caps and mins applied at the wrong time

- 874962 r2 — Sir Taco "+1 Power Per Life Left Max. 8" vs Callie "-1 Opp Power And Damage, Min 1": engine 8, server 7 (cap first, then −1).
- 877950 r1 — Eugene (Power +2 bonus) vs Cindy "-2 Opp Power And Damage, Min 6": engine 8, server 7.

## 7. Defeat / post-round gains after a KO

Server grants nothing to a player whose life hit 0 that round. Engine already blocks heals at
0 life but still grants pillz.

- 876712 r1 — Kubra "Defeat: +1 Pillz And Life" + bonus, KO'd: engine pillz 10, server 9.
- 877023 r1 — same card, same situation: engine 7, server 6.

## 8. Recover N Pillz Out Of M — counts the free pill

- 877983 r1 — Eebiza "Defeat: Recover 1 Pillz Out Of 2" with 0 pillz bet (pillzUsed 1): server +1 (ceil(1×1/2)), engine +0. The server's base is `pillzUsed` (bet + 1).

## 9. Symmetry / Asymmetry index check

- 877308 r3 — Nantosuelte "Asymmetry: Damage +3" vs Pavone: cards were in the same slot, server did not apply, engine did (6 vs 3).

## 10. Unimplemented / unparsed ability keywords

- 875230 r3 — Wilo Ld "Repair 1, Max. 14": permanent +1 life post-round (server `isPermanent: true`); engine life 6, server 7.
- 874795 r0 — El Resbaladizo "Cards Damage +2": engine 6, server 8 (+4 = 2 × something; semantics TBD).
- 874712 r1 — Tina "Revenge: Power And Damage +2" (previous round lost) vs Kochar "Damage Impose": engine damage 2, server 4.
- 877733 r1 — Korakine "Unison : +2 Pillz And Life" (all four cards same clan) lost the round: engine +1 life, server +0.

- 876464 r0–r2 — Tolvack (clan 60, added 2026-09-10) bonus "After [clan:56][clan:60] : Power +3":
  activates once a card of one of the listed clans has been played earlier in the game
  (r1 Drava power 10 = 7 + 3, r2 Maelt Riv 11 = 8 + 3; r0 Tør stays 6). Also Tør "Degrowth:
  -1 Opp Power, Min 2" and Maelt Riv "Consume 1, Min 2". Unimplemented keywords: After, Consume.

## 11. "+1 Attack Per Life Left" uses the wrong life value

- 876939 r3 — Buga Baga Ld bonus with 1 life left: engine 10, server 8 (7×1 + 1).

## Data gaps (not engine bugs)

- Resolved 2026-09-10: clan 60 is **Tolvack** (bonus "After [clan:56][clan:60] : Power +3"); added to `CardTypes.ts`, all 2496 cards / 36 clans now in `data.json`. Totals after that: 53 replayable, 30 exact, 23 mismatched.
- `abilityData` (structured ability description) is only present in battle snapshots, not in the card DB dump. Every card seen in a battle is therefore a sample for a future structured parser.
