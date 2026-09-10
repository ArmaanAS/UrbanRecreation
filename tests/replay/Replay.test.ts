// Replay captured real games (captures/games/*.json, produced by `deno task extract`)
// through the engine and check life / pillz after every round.
//
//   deno test -A tests/replay/            # all captured games
//   deno test -A tests/replay/ -- 866431  # one battle id
import { HandGenerator } from "@/game/Hand.ts";
import Game from "@/game/Game.ts";
import Player from "@/game/Player.ts";
import { Turn } from "@/game/types/Types.ts";
import { HandOf } from "@/game/types/CardTypes.ts";
import { assertEquals } from "@std/assert";

const GAME_DIR = new URL("../../captures/games/", import.meta.url);

interface MoveResult {
  power: number;
  damage: number;
  attack: number;
  won: boolean;
}
interface Testcase {
  cards: string[];
  levels?: number[];
  night?: boolean;
  life: number;
  pillz: number;
  moves: {
    s1: [number, number, boolean];
    s2: [number, number, boolean];
    p1life: number;
    p2life: number;
    p1pillz: number;
    p2pillz: number;
    r1?: MoveResult;
    r2?: MoveResult;
  }[];
}
interface GameRecord {
  id: number;
  players: { name: string }[];
  firstPlayer: 0 | 1 | null;
  finalStatus: string;
  issues: string[];
  testcase: Testcase | null;
}

const only = Deno.args.filter((a) => /^\d+$/.test(a)).map(Number);
const records: GameRecord[] = [];
for await (const f of Deno.readDir(GAME_DIR)) {
  if (!f.name.endsWith(".json")) continue;
  const rec: GameRecord = JSON.parse(await Deno.readTextFile(new URL(f.name, GAME_DIR)));
  if (only.length && !only.includes(rec.id)) continue;
  records.push(rec);
}
records.sort((a, b) => a.id - b.id);

for (const rec of records) {
  const tc = rec.testcase;
  const name = `Replay ${rec.id}: ${rec.players[0]?.name} vs ${rec.players[1]?.name}`;
  Deno.test({ name, ignore: tc === null || rec.finalStatus === "playing", fn: () => {
    const lv = tc!.levels ?? [];
    const h1 = HandGenerator.handOf(tc!.cards.slice(0, 4) as HandOf<string>, lv.slice(0, 4) as HandOf<number | undefined>);
    const h2 = HandGenerator.handOf(tc!.cards.slice(4, 8) as HandOf<string>, lv.slice(4, 8) as HandOf<number | undefined>);
    const p1 = new Player(tc!.life, tc!.pillz, 0);
    const p2 = new Player(tc!.life, tc!.pillz, 1);
    const g = new Game(p1, p2, h1, h2, Turn.PLAYER_1, false, tc!.night ?? false);

    tc!.moves.forEach((move, i) => {
      g.select(move.s1[0], move.s1[1], move.s1[2], false);
      g.select(move.s2[0], move.s2[1], move.s2[2], false);
      // s1 is always the first mover: player 1 in even rounds, player 2 in odd rounds.
      const [m1, m2] = i % 2 === 0 ? [move.s1, move.s2] : [move.s2, move.s1];
      const c1 = g.h1[m1[0]];
      const c2 = g.h2[m2[0]];
      const fmt = (name: string, m: [number, number, boolean]) => `${name} (${m[1]}pz${m[2] ? " fury" : ""})`;
      const label = `round ${i}: ${fmt(tc!.cards[m1[0]], m1)} vs ${fmt(tc!.cards[4 + m2[0]], m2)}`;

      // Per-card outcome of the round (the played cards stay in the hands after resolution).
      const outcome = (c: typeof c1): MoveResult => ({ power: c.power.final, damage: c.damage.final, attack: c.attack.final, won: !!c.won });
      if (move.r1) assertEquals(outcome(c1), move.r1, `${label} — ${tc!.cards[m1[0]]} result`);
      if (move.r2) assertEquals(outcome(c2), move.r2, `${label} — ${tc!.cards[4 + m2[0]]} result`);

      const state = { p1life: g.p1.life, p2life: g.p2.life, p1pillz: g.p1.pillz, p2pillz: g.p2.pillz };
      const expected = { p1life: move.p1life, p2life: move.p2life, p1pillz: move.p1pillz, p2pillz: move.p2pillz };
      assertEquals(state, expected, `${label} — life/pillz`);
    });
  } });
}
