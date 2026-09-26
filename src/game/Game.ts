import Hand, { HandGenerator } from "./Hand.ts";
import "colors";
import { AbilityString, type CardJSON, type Clan, type HandOf } from "./types/CardTypes.ts";
import type Card from "./Card.ts";
import Player from "./Player.ts";
import GameRenderer from "../utils/GameRenderer.ts";
import Events from "./battle/Events.ts";
import PlayerRound from "./PlayerRound.ts";
import CardBattle from "./battle/CardBattle.ts";
import { Turn } from "./types/Types.ts";
import CachedCardBattle from "./battle/CachedCardBattle.ts";
import Ability from "@/game/Ability.ts";
import { DEBUG } from "@/utils/Debug.ts";

export let counter = 0;

export enum Winner {
  PLAYING = 0,
  PLAYER_1 = 1,
  PLAYER_2 = 2,
  TIE = 3,
}

export type CardIndex = 0 | 1 | 2 | 3 | number;
export type Selection = [CardIndex, number, boolean];

/**
 * The two tables a match derives once, at construction, and every position in it reads.
 * They are per match - owned by the Game that built them and shared by reference with every
 * clone, make/unmake and search node taken from it - so any number of Games can be alive
 * and interleaved in one process. Both used to be module globals, which let the most
 * recently built Game silently answer for every other one.
 *
 * They are keyed by symbol so that neither a structured clone (the worker pool's
 * `postMessage`) nor `JSON.stringify` copies them: `Game.from` rebuilds both on the far
 * side, where the cards they compile are no longer the same objects anyway.
 */
const BASES: unique symbol = Symbol("Game.bases");
const BATTLES: unique symbol = Symbol("Game.battles");

/**
 * Everything one `Game.make()` can change, so it can be walked back by `Game.unmake()`.
 *
 * Search allocates one of these per depth and reuses it, so exploring a move costs no
 * allocation at all - which is the point. Cloning the game per node was ~23 objects a time
 * and left GC at 27% of the search; this is a dozen integer writes.
 */
export class Undo {
  id = 0;
  winner = Winner.PLAYING;
  i1: Selection | undefined = undefined;
  i2: Selection | undefined = undefined;
  /** True when this move resolved a round, so a battle has to be undone as well. */
  battled = false;
  hand: Hand | undefined = undefined;
  handIndex = 0;

  p1a = 0;
  p2a = 0;
  r1a = 0;
  r2a = 0;
  r1clan: Clan | undefined = undefined;
  r2clan: Clan | undefined = undefined;

  c1i = 0;
  c2i = 0;
  /** Each side's `Events.mask` before the battle, so unmake restores it exactly. */
  mask1 = 0;
  mask2 = 0;
  c1: Card | undefined = undefined;
  c2: Card | undefined = undefined;

  /** Length of each `Events.repeat[t]` before the battle, per side. */
  rep1: number[] = new Array(10).fill(0);
  rep2: number[] = new Array(10).fill(0);
  /** Latched permanents already in `repeat`, whose flags a battle can flip. */
  perms: Ability[] = [];
  permWon: (boolean | undefined)[] = [];
  permDelayed: (boolean | undefined)[] = [];
  permCount = 0;

  captureRepeat(events: Events, lens: number[]) {
    for (let t = 0; t < 10; t++) {
      const arr = events.repeat[t];
      const n = arr.length;
      lens[t] = n;
      for (let k = 0; k < n; k++) {
        const a = arr[k];
        this.perms[this.permCount] = a;
        this.permWon[this.permCount] = a.won;
        this.permDelayed[this.permCount] = a.delayed;
        this.permCount++;
      }
    }
  }

  /**
   * Permanents merged during the battle are pushed onto the tail of `repeat`, and the only
   * ones `removeGlobal` can take out are those same ones (an already-latched ability has
   * `won === true` and never reaches that branch), so the saved prefix is intact and
   * truncating restores it.
   */
  restoreRepeat(events: Events, lens: number[]) {
    for (let t = 0; t < 10; t++) {
      const arr = events.repeat[t];
      if (arr.length !== lens[t]) arr.length = lens[t];
    }
  }
}

export default class Game {
  id: number;
  p1: Player;
  p2: Player;
  winner = Winner.PLAYING;
  h1: Hand;
  h2: Hand;

  i1?: Selection = undefined;
  i2?: Selection = undefined;

  events1 = new Events();
  events2 = new Events();
  r1: PlayerRound;
  r2: PlayerRound;

  /**
   * Turn order per `id`: 0..8 when player 1 moved first in round one, 9..17 when player 2
   * did (see `createBaseGameCache`). Depends on the hands' Counter-attack Leaders.
   */
  [BASES]: BaseGame[] = [];
  /** A compiled battle per card pair, at `ci1 * 4 + ci2`; see `createBattleDataCache`. */
  [BATTLES]: (CachedCardBattle | undefined)[] = [];

  constructor(
    p1: Player,
    p2: Player,
    h1: Hand,
    h2: Hand,
    first: Turn = Turn.PLAYER_1,
    draw = true,
    /** Clint City is at night: Night: conditions hold and cards use their night ability / bonus. */
    night = false,
  ) {
    this.p1 = p1;
    this.p2 = p2;

    this.h1 = h1;
    this.h2 = h2;

    if (night) {
      for (const hand of [h1, h2]) for (const card of hand) card.night = true;
    }

    this.id = this.createBaseGameCache(first);

    // this.r1 = new PlayerRound(
    //   1, first === Turn.PLAYER_1, p1, h1, p2, h2, this.events1);
    // this.r2 = new PlayerRound(
    //   1, first === Turn.PLAYER_2, p2, h2, p1, h1, this.events2);
    this.r1 = new PlayerRound(
      1,
      first === Turn.PLAYER_1,
      h1,
      h2,
      !night,
    );
    this.r2 = new PlayerRound(
      1,
      first === Turn.PLAYER_2,
      h2,
      h1,
      !night,
    );

    for (const hand of [h1, h2]) {
      for (const card of hand) {
        if (card.clan == "Leader") {
          if (hand.getClanCards(card) > 1) {
            card.ability.string = AbilityString.NO_ABILITY;
          }
        } else {
          if (hand.getClanCards(card) === 1) {
            card.bonus.string = AbilityString.NO_ABILITY;
          }
        }
      }
    }

    this.createBattleDataCache();

    if (draw) this.draw();
  }

  /**
   * Once per search node, so the shape of this matters more than anything else here.
   *
   * It used to build a literal and then `Object.setPrototypeOf` it onto Game.prototype,
   * which drags a finished object onto a new map through the runtime - about 1600x the cost
   * of giving it the right prototype from birth (tests/CardAccess.bench.ts). A V8 profile of
   * `deno task time` put `ObjectSetPrototypeOf` at 29% of the entire search, over half of it
   * from here. Fields are assigned in declaration order so every clone shares one map.
   */
  clone(): Game {
    const h1 = this.h1.clone();
    const h2 = this.h2.clone();

    // Events are only mutated while a round is resolving, so between rounds the parent's
    // can be shared; a clone taken mid-round gets its own.
    let events1 = this.events1;
    let events2 = this.events2;
    if (this.id % 2 === 1) {
      events1 = events1.clone();
      events2 = events2.clone();
    }

    const g: Game = Object.create(Game.prototype);
    g.id = this.id;
    g.p1 = this.p1.clone();
    g.p2 = this.p2.clone();
    g.winner = this.winner;
    g.h1 = h1;
    g.h2 = h2;
    g.i1 = this.i1;
    g.i2 = this.i2;
    g.events1 = events1;
    g.events2 = events2;
    g.r1 = this.r1.clone(h1, h2);
    g.r2 = this.r2.clone(h2, h1);
    // The match's tables, by reference: a clone is a position in the same match.
    g[BASES] = this[BASES];
    g[BATTLES] = this[BATTLES];
    return g;
  }

  /**
   * Apply a move, recording into `u` everything needed to walk it back.
   *
   * The move itself is delegated to `select()` so make/unmake can never drift from the
   * engine's own rules: everything here is capture before and restore after. What that
   * costs is a handful of ints, because the mutable state is already packed - a Player and
   * a PlayerRound are one int each, and the two cards a battle touches are *replaced* in
   * the hand by `CachedCardBattle.play` rather than edited, so undoing them is putting the
   * old references back.
   *
   * Returns false, having changed nothing, if the card was already played.
   */
  make(index: CardIndex, pillz: number, fury: boolean, u: Undo): boolean {
    const hand = this.turn === Turn.PLAYER_1 ? this.h1 : this.h2;
    if (hand[index].played) return false;

    u.id = this.id;
    u.winner = this.winner;
    u.i1 = this.i1;
    u.i2 = this.i2;
    u.hand = hand;
    u.handIndex = index;
    u.permCount = 0;

    const willBattle = this.firstHasSelected;
    u.battled = willBattle;
    if (willBattle) {
      u.p1a = this.p1.snapshot();
      u.p2a = this.p2.snapshot();
      u.r1a = this.r1.snapshot();
      u.r2a = this.r2.snapshot();
      u.r1clan = this.r1.lastClan;
      u.r2clan = this.r2.lastClan;
      // The slots CachedCardBattle.play is about to overwrite with freshly compiled cards.
      u.c1i = this.turn === Turn.PLAYER_1 ? index : this.i1![0];
      u.c2i = this.turn === Turn.PLAYER_2 ? index : this.i2![0];
      u.c1 = this.h1[u.c1i];
      u.c2 = this.h2[u.c2i];
      // Every time that holds anything fires per battle and empties its `events`, so only
      // `repeat` - the latched permanents - survives a round and needs saving, with the
      // mask that says which times to run.
      u.captureRepeat(this.events1, u.rep1);
      u.captureRepeat(this.events2, u.rep2);
      u.mask1 = this.events1.mask;
      u.mask2 = this.events2.mask;
    }

    this.select(index, pillz, fury, false);
    return true;
  }

  /** Reverse the `make` recorded in `u`. */
  unmake(u: Undo) {
    if (u.battled) {
      u.restoreRepeat(this.events1, u.rep1);
      u.restoreRepeat(this.events2, u.rep2);
      this.events1.mask = u.mask1;
      this.events2.mask = u.mask2;
      for (let k = 0; k < u.permCount; k++) {
        const a = u.perms[k];
        a.won = u.permWon[k];
        a.delayed = u.permDelayed[k];
      }
      this.h1[u.c1i] = u.c1!;
      this.h2[u.c2i] = u.c2!;
      // Only the card *this* make marked is unmarked; the first mover's stays played until
      // its own make is walked back.
      u.hand![u.handIndex].played = false;

      this.p1.restore(u.p1a);
      this.p2.restore(u.p2a);
      this.r1.restore(u.r1a);
      this.r2.restore(u.r2a);
      this.r1.lastClan = u.r1clan;
      this.r2.lastClan = u.r2clan;
    } else {
      u.hand![u.handIndex].played = false;
    }

    this.i1 = u.i1;
    this.i2 = u.i2;
    this.winner = u.winner;
    this.id = u.id;
  }

  /**
   * Restore a Game that went through a structured clone. That keeps the object graph but
   * strips prototypes and the symbol-keyed per-match tables, so both are rebuilt here.
   */
  static from(o: Game) {
    Object.setPrototypeOf(o, Game.prototype);

    Object.setPrototypeOf(o.p1, Player.prototype);
    Object.setPrototypeOf(o.p2, Player.prototype);

    Hand.from(o.h1);
    Hand.from(o.h2);

    Events.from(o.events1);
    Events.from(o.events2);

    Object.setPrototypeOf(o.r1, PlayerRound.prototype);
    Object.setPrototypeOf(o.r2, PlayerRound.prototype);

    o.createBaseGameCache();
    o.createBattleDataCache();

    return o;
  }

  get base() {
    return this[BASES][this.id];
  }

  /**
   * The match's two tables, which identify it: every clone and make/unmake of this Game
   * shares them by reference, and a constructed or restored (`Game.from`) Game builds its
   * own. A solver cache that is only valid within one match compares both by identity.
   */
  get matchTables(): readonly [object, object] {
    return [this[BASES], this[BATTLES]];
  }

  get day() {
    return true;
  }

  get ca1() {
    return this.base.counterAttack1;
  }

  get ca2() {
    return this.base.counterAttack2;
  }

  get firstHasSelected() {
    return this.base.firstHasSelected;
  }

  get playingFirst() {
    return this.base.playingFirst;
  }

  get round() {
    return this.base.round;
  }

  get turn() {
    return this.base.turn;
  }

  draw() {
    GameRenderer.draw(this);
  }

  get playingHand() {
    return this.turn === Turn.PLAYER_1 ? this.h1 : this.h2;
  }

  get playingPlayer() {
    return this.turn === Turn.PLAYER_1 ? this.p1 : this.p2;
  }

  get playedCardIndex() {
    if (!this.firstHasSelected) {
      throw new Error("No first move selected");
    }

    if (this.turn === Turn.PLAYER_1) {
      return this.i2![0];
    } else {
      return this.i1![0];
    }
  }

  get playedHand() {
    return this.turn === Turn.PLAYER_1 ? this.h2 : this.h1;
  }

  get isPlaying() {
    return this.winner === Winner.PLAYING;
  }

  get unplayedCardIndexes() {
    const indexes: number[] = [];
    const hand = this.playingHand;
    for (let i = 0; i < 4; i++) {
      if (!hand[i].played) {
        indexes.push(i);
      }
    }
    return indexes;
  }

  deselect(draw = true) {
    if (!this.firstHasSelected) {
      console.log("Cannot deselect before first move");
      return;
    }

    this.id--;

    let index: number;
    if (this.turn === Turn.PLAYER_1) {
      index = this.i1![0];
      this.h1[index].played = false;
      this.i1 = undefined;
    } else {
      index = this.i2![0];
      this.h2[index].played = false;
      this.i2 = undefined;
    }

    if (draw) this.draw();

    return index;
  }

  select(index: CardIndex, pillz: number, fury = false, draw = true) {
    // if (typeof index != 'number' || typeof pillz != 'number') //return false;
    if (!Number.isInteger(index) || !Number.isInteger(pillz)) {
      throw new Error(`Game.select - index or pillz is not a number
        index: ${index}, pillz: ${pillz}`);
    }

    // if (this.firstHasSelected != this.first) {
    if (this.turn === Turn.PLAYER_1) {
      // if (this.h1.get(index).won !== undefined)
      // if (this.h1[index].won !== undefined)
      if (this.h1[index].played) {
        return false;
      }

      this.i1 = [index, pillz, fury];
      this.h1[index].played = true;
    } else {
      // if (this.h2.get(index).won !== undefined)
      // if (this.h2[index].won !== undefined)
      if (this.h2[index].played) {
        return false;
      }

      this.i2 = [index, pillz, fury];
      this.h2[index].played = true;
    }

    if (this.firstHasSelected) {
      this.battle();
      this.i1 = undefined;
      this.i2 = undefined;
    } else {
      this.id++;
    }

    if (draw) this.draw();
    return true;
  }

  battle() {
    if (this.i1 !== undefined && this.i2 !== undefined) {
      const card1 = this.h1[this.i1[0]];
      const card2 = this.h2[this.i2[0]];
      const pillz1 = this.i1[1];
      const pillz2 = this.i2[1];
      const fury1 = this.i1[2];
      const fury2 = this.i2[2];

      const ccb = this[BATTLES][this.i1[0] * 4 + this.i2[0]];
      if (ccb === undefined) {
        new CardBattle(
          this,
          this.p1,
          card1,
          pillz1,
          fury1,
          this.p2,
          card2,
          pillz2,
          fury2,
          this.events1,
          this.events2,
        );

        console.error(`CardBattle "${this.i1[0]} ${this.i2[0]}" is not cached`);
      } else {
        ccb.play(this, pillz1, fury1, pillz2, fury2);
      }

      counter++;

      this.nextRound();

      // Both players can hit 0 in the same round - the round loser takes card damage while
      // the winner pays a Backlash / poison cost - and that has to be recorded as a result
      // like any other. Leaving `winner` on PLAYING here let the solver treat a finished
      // game as live: it kept expanding, `id` walked off the end of this half of the
      // base table into the other one (resetting `round` to 1), and the leaves it built
      // had neither children nor a result, so `Node.rating()` returned its Infinity
      // sentinel and every MAX ancestor inherited it.
      if (this.p1.life <= 0 && this.p2.life <= 0) {
        this.winner = Winner.TIE;
      } else if (this.p1.life <= 0) {
        this.winner = Winner.PLAYER_2;
      } else if (this.p2.life <= 0) {
        this.winner = Winner.PLAYER_1;
      } else if (this.round >= 5) {
        if (this.p1.life > this.p2.life) {
          this.winner = Winner.PLAYER_1;
        } else if (this.p1.life < this.p2.life) {
          this.winner = Winner.PLAYER_2;
        } else {
          this.winner = Winner.TIE;
        }
      }
    }
    return;
  }

  input(repeat = true): string[] {
    const msg = `\n
         _____      _           _                       _ 
        /  ___|    | |         | |                     | |
        \\ \`--.  ___| | ___  ___| |_    ___ __ _ _ __ __| |
         \`--. \\/ _ \\ |/ _ \\/ __| __|  / __/ _\` | '__/ _\` |
        /\\__/ /  __/ |  __/ (__| |_   |(_| (_| | | | (_| |
        \\____/ \\___|_|\\___|\\___|\\__|  \\___\\__,_|_|  \\__,_| o o o\n\n\n`;

    console.log(msg.green);
    const answer = ask('Index Pillz [Fury] Or "Undo" or "End" -')!.toLowerCase();

    if (answer === "undo") {
      this.deselect();
      return this.input(repeat);
    } else if (answer === "end") {
      this.p1.life = -1;
      this.p2.life = -1;
      this.id = 7;
      return [];
      // throw new Error("Ending game");
    }

    const s = answer.trim().split(" ");

    const index = +s[0];
    const pillz = +(s[1] ?? 0);
    const fury = s[2] === "true";
    if (index < 0 || index > 3 || pillz < 0) {
      console.log("Invalid input");
      return this.input(repeat);
    }

    if (!this.select(index, pillz, fury)) {
      console.log("Selection failed");
      return this.input(repeat);
    }
    if (this.hasWinner(repeat)) {
      return s;
    }
    if (repeat) {
      return this.input(true);
    }
    // play(this);
    return s;
  }

  hasWinner(log = false) {
    if (this.round > 4 || this.p1.life <= 0 || this.p2.life <= 0) {
      if (log) {
        this.draw();

        if (this.p1.life > this.p2.life) {
          console.log("\n  Game over!\n".white.bgGreen);
          console.log(` ${` ${this.p1.name} `.white.bgCyan} won the match!\n`);
        } else if (this.p1.life < this.p2.life) {
          console.log("\n  Game over!\n".white.bgRed);
          console.log(` ${` ${this.p2.name} `.white.bgCyan} won the match!\n`);
        } else {
          console.log("\n  Game over!\n".white.bgYellow);
          console.log("Game is a draw!\n".green);
        }
      }

      return true;
    }

    return false;
  }

  private nextRound() {
    this.id++;

    this.r1.next(this.playingFirst === Turn.PLAYER_1);
    this.r2.next(this.playingFirst === Turn.PLAYER_2);
  }

  /**
   * Compile a battle for every pair of cards still in hand into this match's own table.
   * Clones share it; a fresh Game (or `Game.from`) builds its own. A pair whose card has
   * already been played is left empty, since it can never battle again.
   */
  createBattleDataCache() {
    // Filled by push, not `new Array(16)`, so the hot lookup reads a packed array.
    const battles: (CachedCardBattle | undefined)[] = [];
    for (let k = 0; k < 16; k++) battles.push(undefined);

    let counter = 0;
    for (let ci1 = 0; ci1 < 4; ci1++) {
      const c1 = this.h1[ci1];
      if (c1.won !== undefined) continue;

      for (let ci2 = 0; ci2 < 4; ci2++) {
        const c2 = this.h2[ci2];
        if (c2.won !== undefined) continue;

        counter++;
        battles[ci1 * 4 + ci2] = new CachedCardBattle(
          this.h1,
          c1,
          this.h2,
          c2,
        );
      }
    }

    this[BATTLES] = battles;
    // Construction narration, behind the same switch as the rest of the engine's tracing:
    // `deno task run` sets UR_DEBUG and still prints it, and nothing else that builds a
    // Game (tests, benches, the advisor's replays) gets a line per construction.
    if (DEBUG) console.log(`Cached ${`${counter}`.green} CardBattles`.white);
  }

  /**
   * Build this match's turn-order table and return the `id` its first position starts at.
   *
   * The table is two runs of nine: `id` 0..8 is the match where player 1 moves first in
   * round one, 9..17 the one where player 2 does, and within a run `id = 2 * (round - 1) +
   * (first mover has selected ? 1 : 0)`, with 8 (and 17) the finished position after round
   * four. `select`/`nextRound`/`deselect` only ever step `id` by one and a finished game
   * stops, so a position stays in the run it started in (only the interactive "end" command
   * in `input` jumps to 7, and it ends the game). What the entries depend on beyond `id` is
   * only whether each hand's Leader is Counter-attack, so the table is per match.
   */
  createBaseGameCache(first = Turn.PLAYER_1) {
    // Written in index order from empty, so it stays a packed array.
    const bases: BaseGame[] = [];
    let counterAttack1 = false;
    const l1 = this.h1.getLeader();
    if (l1?.abilityString == "Counter-attack") {
      counterAttack1 = true;
    }

    let counterAttack2 = false;
    const l2 = this.h2.getLeader();
    if (l2?.abilityString == "Counter-attack") {
      counterAttack2 = true;
    }

    let start = 0;
    for (const f of [Turn.PLAYER_1, Turn.PLAYER_2]) {
      let playingFirst = f;

      for (let i = 0; i <= 8; i++) {
        const firstHasSelected = i % 2 === 1;
        const turn = playingFirst === Turn.PLAYER_1
          ? (!firstHasSelected ? Turn.PLAYER_1 : Turn.PLAYER_2)
          : (!firstHasSelected ? Turn.PLAYER_2 : Turn.PLAYER_1);

        bases[start + i] = {
          day: true,
          round: Math.floor(i / 2) + 1,
          counterAttack1,
          counterAttack2,
          playingFirst,
          firstHasSelected,
          turn,
        };

        if (i % 2 === 1) {
          if (counterAttack1 && !counterAttack2) {
            playingFirst = Turn.PLAYER_2;
          } else if (counterAttack2 && !counterAttack1) {
            playingFirst = Turn.PLAYER_1;
          } else {
            playingFirst = playingFirst === Turn.PLAYER_1
              ? Turn.PLAYER_2
              : Turn.PLAYER_1;
          }
        }
      }

      start += 9;
    }

    this[BASES] = bases;
    return first === Turn.PLAYER_1 ? 0 : 9;
  }
}

interface BaseGame {
  playingFirst: Turn;
  firstHasSelected: boolean;
  turn: Turn;
  counterAttack1: boolean;
  counterAttack2: boolean;
  day: boolean;
  round: number;
}

/**
 * The window-only `prompt`, for interactive play on the main thread. The search workers
 * load this module too, and Deno type-checks a worker against its worker lib, which has no
 * `prompt`; naming it directly made every worker fail to start under a type-checked
 * `deno test`, silently dropping ParallelSearch to one thread.
 */
function ask(message: string): string | null {
  const { prompt } = globalThis as {
    prompt?: (message?: string, defaultValue?: string) => string | null;
  };
  if (prompt === undefined) {
    throw new Error("Interactive input needs the main thread's prompt");
  }
  return prompt(message, "");
}

export class GameGenerator {
  static create() {
    const p1 = new Player(12, 12, 0); // "Player");
    const p2 = new Player(12, 12, 1); // "Urban Rival");

    const h1 = HandGenerator.generate("Roderick", "Frank", "Katsuhkay", "Oyoh"); // Roderick
    // let h1 = Hand.generate('Frank', 'Katsuhkay', 'Frank', 'Katsuhkay'); // Roderick
    const h2 = HandGenerator.generate("Behemoth Cr", "Vholt", "Eyrik", "Kate");

    // 'Jessie', 'Timber'
    // 'gwen', 'Cassio Cr'

    return new Game(p1, p2, h1, h2, Turn.PLAYER_1);
  }

  static createUnique(
    h1: HandOf<CardJSON>,
    h2: HandOf<CardJSON>,
    life: number,
    pillz: number,
    turn?: Turn,
  ) {
    const p1 = new Player(life, pillz, 0); // name1);
    const p2 = new Player(life, pillz, 1); // name2);

    return new Game(
      p1,
      p2,
      HandGenerator.generateRaw(h1),
      HandGenerator.generateRaw(h2),
      turn,
    );
  }
}
