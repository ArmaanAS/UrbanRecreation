// What a Training driver pays per candidate round (src/situations/Situations.ts):
//
//   deno bench -A --no-check tests/situations/Situations.bench.ts
//
// Positions are the start of a pseudo-random captured round of the first 120 replay-ready
// captures; each asks about 100 pseudo-random legal (move, reply) pairs, once through a
// probe per position and once through `situationsOfRound`, which builds a probe per call.
import { gameOf, loadRecords, replayReady } from "@/situations/Corpus.ts";
import {
  LatchLedger,
  type Move,
  RoundProbe,
  situationsOfRound,
  slotIds,
  type SlotIds,
} from "@/situations/Situations.ts";
import type Game from "@/game/Game.ts";
import { Turn } from "@/game/types/Types.ts";

let seed = 7;
const rand = () => (seed = (seed * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff;

function legal(game: Game, side: 0 | 1): Move[] {
  const hand = side === 0 ? game.h1 : game.h2;
  const pillz = side === 0 ? game.p1.pillz : game.p2.pillz;
  const out: Move[] = [];
  for (let i = 0; i < 4; i++) {
    if (hand[i].played) continue;
    for (let p = 0; p <= pillz; p++) {
      out.push([i, p, false]);
      if (p + 3 <= pillz) out.push([i, p, true]);
    }
  }
  return out;
}

const positions: { game: Game; ids: SlotIds; ledger: LatchLedger; pairs: [Move, Move][] }[] = [];
const records = (await loadRecords(new URL("../../captures/games/", import.meta.url)))
  .filter(replayReady)
  .slice(0, 120);
for (const rec of records) {
  const tc = rec.testcase!;
  const game = gameOf(tc);
  const first = rec.firstPlayer!;
  const ids = slotIds(game, [...rec.players[first].hand, ...rec.players[1 - first].hand]);
  const ledger = new LatchLedger();
  const upto = Math.floor(rand() * tc.moves.length);
  for (let r = 0; r < upto; r++) {
    const m = tc.moves[r];
    const [a, b]: [Move, Move] = game.playingFirst === Turn.PLAYER_1 ? [m.s1, m.s2] : [m.s2, m.s1];
    ledger.play(game, a, b, ids);
  }
  if (!game.isPlaying) continue;
  const mine = legal(game, 0), theirs = legal(game, 1);
  const pairs: [Move, Move][] = [];
  for (let k = 0; k < 100; k++) {
    pairs.push([mine[Math.floor(rand() * mine.length)], theirs[Math.floor(rand() * theirs.length)]]);
  }
  positions.push({ game, ids, ledger, pairs });
}

Deno.bench("one probe per position, 100 rounds each", { group: "situations", baseline: true }, () => {
  for (const { game, ids, ledger, pairs } of positions) {
    const probe = new RoundProbe(game, { ids, ledger });
    for (const [a, b] of pairs) probe.situations(a, b);
  }
});

Deno.bench("situationsOfRound per round", { group: "situations" }, () => {
  for (const { game, ids, ledger, pairs } of positions) {
    for (const [a, b] of pairs) situationsOfRound(game, a, b, { ids, ledger });
  }
});
