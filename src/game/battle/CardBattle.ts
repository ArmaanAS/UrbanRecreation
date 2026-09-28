import BattleData from "./BattleData.ts";
import Card from "../Card.ts";
import Events from "./Events.ts";
import Game from "../Game.ts";
import Player from "../Player.ts";
import { Turn } from "../types/Types.ts";
import EventTime from "@/game/types/EventTime.ts";
import { DEBUG } from "../../utils/Debug.ts";
export default class CardBattle {
  constructor(
    game: Game,
    p1: Player,
    card1: Card,
    pillz1: number,
    fury1: boolean,
    p2: Player,
    card2: Card,
    pillz2: number,
    fury2: boolean,
    events1: Events,
    events2: Events,
    compile = true,
  ) {
    const totalPillz1 = pillz1 + (fury1 ? 3 : 0);
    const totalPillz2 = pillz2 + (fury2 ? 3 : 0);
    // this.
    const b1 = new BattleData(
      game.r1,
      p1,
      card1,
      totalPillz1,
      p2,
      card2,
      totalPillz2,
      events1,
      compile,
      fury1,
    );
    // this.
    const b2 = new BattleData(
      game.r2,
      p2,
      card2,
      totalPillz2,
      p1,
      card1,
      totalPillz1,
      events2,
      compile,
      fury2,
    );

    b1.other = b2;
    b2.other = b1;

    // CardBattle.battle(
    //   game, p1, card1, pillz1, fury1,
    //   p2, card2, pillz2, fury2,
    //   events1, events2, b1, b2
    // )
    // A time runs only if its `Events.mask` bit says it might hold something. An empty time
    // is a no-op, and a battle usually fills two or three of the twenty, so the other calls
    // were pure overhead: skipping them took `deno task time-search` from 0.64 s to 0.51 s.
    p1.wonPrevious = p1.won;
    p2.wonPrevious = p2.won;

    // events1.executePre(b1);
    // events2.executePre(b2);
    if (events1.mask & (1 << EventTime.START)) events1.execute(EventTime.START, b1);
    if (events2.mask & (1 << EventTime.START)) events2.execute(EventTime.START, b2);
    // console.log("Executing pre", 4);
    if ((events1.mask | events2.mask) & (1 << EventTime.PRE4)) {
      Events.executeCancels(events1, b1, events2, b2);
    }
    // console.log("Executing pre", 3);
    if (events1.mask & (1 << EventTime.PRE3)) events1.execute(EventTime.PRE3, b1);
    if (events2.mask & (1 << EventTime.PRE3)) events2.execute(EventTime.PRE3, b2);
    // console.log("Executing pre", 2);
    if (events1.mask & (1 << EventTime.PRE2)) events1.execute(EventTime.PRE2, b1);
    if (events2.mask & (1 << EventTime.PRE2)) events2.execute(EventTime.PRE2, b2);
    // console.log("Executing pre", 1);
    // The reductions of both sides together, by descending Min (Events.executeCuts).
    if ((events1.mask | events2.mask) & (1 << EventTime.PRE1)) {
      Events.executeCuts(EventTime.PRE1, events1, b1, events2, b2);
    }

    const a1 = card1.power.final * (pillz1 + 1);
    const a2 = card2.power.final * (pillz2 + 1);
    card1.attack.final = a1;
    card2.attack.final = a2;
    // The Attack each card would have on one pill fewer, which every Attack modifier below also
    // writes (`BattleData.fewerAttack`, read by Perfect).
    b1.fewerAttack = card1.power.final * pillz1;
    b2.fewerAttack = card2.power.final * pillz2;

    // events1.executePost(b1);
    // events2.executePost(b2);
    if (events1.mask & (1 << EventTime.POST1)) events1.execute(EventTime.POST1, b1);
    if (events2.mask & (1 << EventTime.POST1)) events2.execute(EventTime.POST1, b2);
    if ((events1.mask | events2.mask) & (1 << EventTime.POST2)) {
      Events.executeCuts(EventTime.POST2, events1, b1, events2, b2);
    }
    if (events1.mask & (1 << EventTime.POST3)) events1.execute(EventTime.POST3, b1);
    if (events2.mask & (1 << EventTime.POST3)) events2.execute(EventTime.POST3, b2);
    if (events1.mask & (1 << EventTime.POST4)) events1.execute(EventTime.POST4, b1);
    if (events2.mask & (1 << EventTime.POST4)) events2.execute(EventTime.POST4, b2);

    // Fury is settled where the Damage is dealt, not alongside the card's own Damage
    // modifiers: the Attack phase above and the POST modifiers still see the printed
    // Damage. Goran's "+2 Attack Per Opp. Damage" read 2, not 4, against a Fury Uuber in
    // battle 1130726 r3 (8x4 + 2x2 - Hive Equalizer 3x4 = 24). The END effects and the
    // life loss below still count it.
    if (fury1) {
      card1.damage.final += 2;
    }

    if (fury2) {
      card2.damage.final += 2;
    }

    if (DEBUG) {
      console.log(
        `\t\t\t\t\t${` ${p2.name} `.bgBlue.white} Attack ${card2.attack.final}\
      |       ${` ${p1.name} `.bgBlue.white} Attack ${card1.attack.final}`
          .white,
      );
    }

    p1.pillz -= totalPillz1;
    p2.pillz -= totalPillz2;

    const attack1 = card1.attack.final;
    const attack2 = card2.attack.final;
    // A tie goes to a lone Solomon's owner (Tie-break): 1506259 r3 has Solomon at 6 x 1 beat
    // a Fury Hammer Cr at 6 x 1 of the same level, though Hammer Cr moved first. Otherwise the
    // fewer stars win, then the round's first mover.
    const tieBreaker = game.tieBreaker;
    if (
      attack1 > attack2 ||
      (attack1 === attack2 && (tieBreaker !== undefined
        ? tieBreaker === Turn.PLAYER_1
        : card1.stars < card2.stars ||
          (card1.stars === card2.stars &&
            game.playingFirst === Turn.PLAYER_1)))
    ) {
      // console.log(`Life -${card1.damage.final}`);
      p1.won = card1.won = true;
      p2.won = card2.won = false;
      p2.life -= card1.damage.final;
    } else {
      // console.log(`Life -${card2.damage.final}`);
      p1.won = card1.won = false;
      p2.won = card2.won = true;
      p1.life -= card2.damage.final;
    }

    // The end of the round settles the round's own increases, then its decreases by
    // descending Min, then the latched permanents (Events.executeEnd, docs/replay-triage.md).
    if ((events1.mask | events2.mask) & (1 << EventTime.END)) {
      Events.executeEnd(events1, b1, events2, b2);
    }

    card1.played = true;
    card2.played = true;

    // "After [clan:...]" looks at what its owner played in the *previous* round, so record
    // that only now, with the round resolved. b1 and b2 are the two players' views of it.
    // It is the printed clan: "The Oculus, even when infiltrated into the Frozn clan, do not
    // activate this condition", as every After text that names another clan says
    // (captures/abilities.json 5602, 5670, 5681, ...), while [clan:56] ones name the Oculus.
    b1.round.lastClan = card1.baseClan;
    b2.round.lastClan = card2.baseClan;
  }
}
