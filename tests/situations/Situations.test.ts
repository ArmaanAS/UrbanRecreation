// Situation keys by ablation (src/situations/Situations.ts): hand-built rounds whose sources
// are known to fire or not, an interaction, a latched permanent paying, and the probe's
// promise to leave the position it was given alone.
import { assert, assertEquals, assertFalse, assertThrows } from "@std/assert";
import Game from "@/game/Game.ts";
import { HandGenerator } from "@/game/Hand.ts";
import Player from "@/game/Player.ts";
import { Turn } from "@/game/types/Types.ts";
import type { HandOf } from "@/game/types/CardTypes.ts";
import {
  LatchLedger,
  noveltyOf,
  RoundProbe,
  situationsOfRound,
  slotIds,
} from "@/situations/Situations.ts";

// Every card is the lone member of its clan in its hand, so no clan bonus is live and each
// round's sources are exactly the abilities named here.
//   P1: Aurora L5 7/5 "+3 Life" (377), Frogo L2 6/3 "Defeat : Heal 1 Max. 13" (1625),
//       Lyse Teria Cr L2 7/2 "Stop Opp. Ability" (73), Blackie L3 3/5
//   P2: Tina L3 5/4 "Revenge: Power And Damage +2" (883), Miyo L3 6/5 "Stop Opp. Bonus"
//       (1359), Orka Cr L4 6/6 "Degrowth: -1 Opp Damage, Min 1" (1582), Lagertha Cr L4 5/6
const game = () =>
  new Game(
    new Player(12, 12, 0),
    new Player(12, 12, 1),
    HandGenerator.handOf(
      ["Aurora", "Frogo", "Lyse Teria Cr", "Blackie"] as HandOf<string>,
      [5, 2, 2, 3] as HandOf<number | undefined>,
    ),
    HandGenerator.handOf(
      ["Tina", "Miyo", "Orka Cr", "Lagertha Cr"] as HandOf<string>,
      [3, 3, 4, 4] as HandOf<number | undefined>,
    ),
    Turn.PLAYER_1,
    false,
  );

const firedIds = (keys: Set<string>) => [...keys].filter((k) => k.startsWith("fired:"));

Deno.test("an ability that changes the round fires, with its owner's context", () => {
  const g = game();
  const ids = slotIds(g);
  assertEquals(ids.ability.slice(0, 3), [377, 1625, 73]);
  assertEquals(ids.bonus, [undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined]);

  // Aurora on 5 pillz (7 x 6 = 42) beats Miyo on none (6): +3 Life lands on a win.
  const s = new RoundProbe(g).analyse([0, 5, false], [1, 0, false]);
  assertEquals(s.p1, { power: 7, damage: 5, attack: 42, won: true });
  assertEquals(s.life, [15, 7]);
  assertEquals(
    firedIds(s.keys).sort(),
    ["fired:ability:377", "fired:ability:377:win:first"],
  );
  // Miyo's Stop Opp. Bonus had no bonus to stop: present, but it changed nothing.
  assertFalse(s.keys.has("fired:ability:1359"));
});

Deno.test("a Revenge with nothing to avenge does not fire; after a lost round it does", () => {
  const g = game();
  const ids = slotIds(g);
  // Round one: no previous round, so Tina's Revenge cannot hold.
  const first = situationsOfRound(g, [0, 5, false], [0, 0, false], { ids });
  assertFalse([...first].some((k) => k.startsWith("fired:ability:883")));

  // Player 2 loses round one with Miyo, so Tina's Revenge holds in round two, where player 2
  // moves first: 7 x 4 = 28 against Frogo's 6.
  const ledger = new LatchLedger();
  ledger.play(g, [0, 5, false], [1, 0, false], ids);
  const second = new RoundProbe(g, { ids, ledger }).analyse([1, 0, false], [0, 3, false]);
  assertEquals(second.p2, { power: 7, damage: 6, attack: 28, won: true });
  assert(second.keys.has("fired:ability:883"));
  assert(second.keys.has("fired:ability:883:win:first"));
  // Frogo's Defeat: Heal latches on the loss and pays nothing yet: it fires on the latch.
  assert(second.keys.has("fired:ability:1625:lose:second"));
});

Deno.test("a Stop that blocks an ability fires, the stopped one does not, and they pair", () => {
  const g = game();
  const ids = slotIds(g);
  const ledger = new LatchLedger();
  ledger.play(g, [0, 5, false], [1, 0, false], ids); // player 2 loses round one

  // Round two: Lyse Teria Cr's Stop Opp. Ability meets Tina's live Revenge.
  const s = new RoundProbe(g, { ids, ledger }).analyse([2, 0, false], [0, 3, false]);
  assertEquals(s.p2, { power: 5, damage: 4, attack: 20, won: true });
  assert(s.keys.has("fired:ability:73"));
  assert(s.keys.has("fired:ability:73:lose:second"));
  assertFalse(s.keys.has("fired:ability:883"));
  assertEquals(s.pairs, ["pair:opp:ability:73|ability:883"]);
});

Deno.test("a latched permanent pays in a later round, attributed with or without a ledger", () => {
  const g = game();
  const ids = slotIds(g);
  const ledger = new LatchLedger();
  ledger.play(g, [0, 5, false], [1, 0, false], ids); // P1 15 Life, P2 7
  ledger.play(g, [1, 0, false], [0, 3, false], ids); // Frogo loses to Tina: 9 Life, Heal latches
  assertEquals(g.p1.life, 9);
  assertEquals(ledger.origins[0][9], [{ slot: "ability", id: 1625 }]);

  const withLedger = new RoundProbe(g, { ids, ledger }).analyse([2, 0, false], [2, 0, false]);
  assertEquals(withLedger.life[0], 10); // the Heal's +1
  assert(withLedger.keys.has("paid:ability:1625"));
  assertEquals(withLedger.fired[0].includes("paid:ability:1625"), true);

  // Without one, the entry is matched to Frogo by its compiled text.
  const byText = situationsOfRound(g, [2, 0, false], [2, 0, false], { ids });
  assert(byText.has("paid:ability:1625"));
});

Deno.test("the probe leaves the game alone and its baseline is the engine's own round", () => {
  const g = game();
  const ids = slotIds(g);
  const ledger = new LatchLedger();
  ledger.play(g, [0, 5, false], [1, 0, false], ids);
  ledger.play(g, [1, 0, false], [0, 3, false], ids);

  const state = (game: Game) =>
    JSON.stringify({
      id: game.id,
      players: [game.p1.snapshot(), game.p2.snapshot(), game.r1.snapshot(), game.r2.snapshot()],
      hands: [game.h1, game.h2],
      latches: [game.events1, game.events2].map((e) => [
        e.mask,
        e.repeat.map((arr) => arr.map((a) => [a.ability, a.won, a.delayed])),
      ]),
    });
  const before = state(g);
  const probe = new RoundProbe(g, { ids, ledger });
  // The cards left: Lyse Teria Cr and Blackie against Orka Cr and Lagertha Cr.
  for (const p1 of [2, 3]) {
    for (const p2 of [2, 3]) probe.analyse([p1, 1, false], [p2, 2, p2 === 3]);
  }
  const s = probe.analyse([3, 2, false], [3, 1, true]);
  assertEquals(state(g), before);

  ledger.play(g, [3, 2, false], [3, 1, true], ids);
  const result = (i: number, h: typeof g.h1) => ({
    power: h[i].power.final,
    damage: h[i].damage.final,
    attack: h[i].attack.final,
    won: !!h[i].won,
  });
  assertEquals(s.p1, result(3, g.h1));
  assertEquals(s.p2, result(3, g.h2));
  assertEquals(s.life, [g.p1.life, g.p2.life]);
  assertEquals(s.pillz, [g.p1.pillz, g.p2.pillz]);
});

Deno.test("an illegal move is refused rather than resolved", () => {
  const g = game();
  const probe = new RoundProbe(g);
  assertThrows(() => probe.analyse([0, 13, false], [0, 0, false]), Error, "cannot bet");
  assertThrows(() => probe.analyse([0, 10, true], [0, 0, false]), Error, "cannot bet");
  assertThrows(() => probe.analyse([4, 0, false], [0, 0, false]), Error, "cannot play");
  g.select(0, 0, false, false);
  assertThrows(() => new RoundProbe(g), Error, "start of a round");
});

Deno.test("novelty is the sum of 1 / (1 + n) over the keys", () => {
  const counts = { "fired:ability:1": 0, "fired:ability:2": 1, "fired:ability:3": 3 };
  assertEquals(
    noveltyOf(["fired:ability:2", "fired:ability:3", "fired:ability:9"], counts),
    1 / 2 + 1 / 4 + 1,
  );
  assertEquals(noveltyOf([], counts), 0);
});
