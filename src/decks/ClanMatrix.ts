// Clans against each other, from the hands actually played in a format (phase 6 of
// docs/deck-builder-design.md). "In theory": hand pairs drawn from two clans' captured hands are
// solved exactly on the Rust matchup binary, both first movers, scored like deckVsDeck. "In
// practice": how hands of those clans did in the captured games themselves. Both use the same
// hands, which come from the owner's own matchmaking, so every number carries that bias, and one
// side of every captured game is the owner.
//
// A hand belongs to a clan when at least three of its four cards do, so its clan bonus is live
// in most rounds; other hands are left out.
import { type DeckCardRef, type HandPair, isRefusedPair, type PairOutcome, solvePairs, type SolveOptions, summarize } from "./Matchup.ts";
import { describeRefusal } from "./Coverage.ts";
import type { GameRecord } from "./Meta.ts";

/** The parts of a captures/games/<id>.json record this reads, beyond Meta's. */
export interface MatrixGameRecord extends GameRecord {
  night?: boolean;
  result?: { result?: string } | null;
  players: (GameRecord["players"][number] & { name?: string })[];
}

export interface FieldHand {
  hand: DeckCardRef[];
  clan: string;
  gameId: number;
  player: number | string;
  owner: boolean;
  /** 1 won, 0 lost, 0.5 drawn, null when the capture has no result. */
  score: number | null;
}

/** The clan three or four of a hand's cards share, or null. */
export function handClan(hand: readonly { clan?: string | null }[]): string | null {
  const counts = new Map<string, number>();
  for (const card of hand) if (card.clan) counts.set(card.clan, (counts.get(card.clan) ?? 0) + 1);
  for (const [clan, count] of counts) if (count >= 3) return clan;
  return null;
}

const RESULT_SCORE: Record<string, number> = { win: 1, lose: 0, draw: 0.5 };

/** Every clan-labelled four-card hand of a format's captured games, both sides. */
export function fieldHands(games: readonly MatrixGameRecord[], formatId: number, ownerId?: number): FieldHand[] {
  const out: FieldHand[] = [];
  for (const game of games) {
    if ((game.room?.idDeckFormat ?? -1) !== formatId) continue;
    const mine = typeof game.mySide === "number"
      ? game.mySide
      : game.players.find((p) => p.id === (game.myId ?? ownerId))?.side ?? null;
    const ownerScore = RESULT_SCORE[game.result?.result ?? ""] ?? null;
    for (const player of game.players) {
      const clan = player.hand.length === 4 ? handClan(player.hand) : null;
      if (!clan) continue;
      const owner = player.side === mine;
      out.push({
        hand: player.hand.map(({ id, level }) => ({ id, level })),
        clan,
        gameId: game.id,
        player: player.id ?? player.name ?? `${game.id}:${player.side}`,
        owner,
        score: ownerScore === null || mine === null ? null : owner ? ownerScore : 1 - ownerScore,
      });
    }
  }
  return out;
}

/** 32-bit FNV-1a of a string, so a cell's hands depend on its clans' names, not their rank. */
function hashText(text: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) h = Math.imul(h ^ text.charCodeAt(i), 0x01000193);
  return h >>> 0;
}

function stream(...words: number[]): () => number {
  let a = words.reduce((h, w) => Math.imul(h ^ (w >>> 0), 0x9e3779b1) >>> 0, 0x2545f491);
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export interface ClanGroup {
  clan: string;
  hands: FieldHand[];
  /** Distinct players among those hands, and how many of the hands were the owner's. */
  players: number;
  ownerHands: number;
}

/** Clans with at least `minHands` hands, most played first. */
export function clanGroups(hands: readonly FieldHand[], minHands: number): ClanGroup[] {
  const byClan = new Map<string, FieldHand[]>();
  for (const hand of hands) byClan.set(hand.clan, [...(byClan.get(hand.clan) ?? []), hand]);
  return [...byClan]
    .filter(([, list]) => list.length >= minHands)
    .map(([clan, list]) => ({
      clan,
      hands: list,
      players: new Set(list.map((h) => h.player)).size,
      ownerHands: list.filter((h) => h.owner).length,
    }))
    .sort((x, y) => y.hands.length - x.hands.length || x.clan.localeCompare(y.clan));
}

/** `perCell` pairs of a hand of clan A against a hand of clan B, each drawn with replacement. */
export function cellPairs(a: ClanGroup, b: ClanGroup, perCell: number, seed: number): HandPair[] {
  const pickA = stream(seed, hashText(a.clan), hashText(b.clan), 0xa);
  const pickB = stream(seed, hashText(a.clan), hashText(b.clan), 0xb);
  return Array.from({ length: perCell }, () => ({
    a: a.hands[Math.floor(pickA() * a.hands.length)].hand,
    b: b.hands[Math.floor(pickB() * b.hands.length)].hand,
  }));
}

export interface CellResult {
  a: string;
  b: string;
  scored: number;
  refused: number;
  /** A's mean score in [-1, 1] and its standard error; NaN when nothing (or one pair) scored. */
  mean: number;
  stderr: number;
  /** Captured games between a hand of A and a hand of B, and A's score in them. */
  practice: { games: number; score: number };
}

export interface ClanResult {
  clan: string;
  hands: number;
  players: number;
  ownerHands: number;
  /** Mean of its cells, each opponent clan weighted equally, with the standard error. */
  vsClans: number;
  vsClansErr: number;
  /** Mean of its cells weighted by how often the owner's opponents play each clan: the field. */
  vsField: number;
  vsFieldErr: number;
  scored: number;
  refused: number;
  /** Captured games of its hands against any labelled hand, and their score. */
  practice: { games: number; score: number };
}

export interface ClanMatrixResult {
  perCell: number;
  seed: number;
  night: boolean;
  clans: ClanResult[];
  cells: CellResult[];
  refusals: { reason: string; count: number }[];
  cached: number;
  solved: number;
}

function weighted(values: { mean: number; stderr: number; weight: number }[]) {
  const usable = values.filter((v) => Number.isFinite(v.mean) && v.weight > 0);
  const total = usable.reduce((s, v) => s + v.weight, 0);
  if (!total) return { mean: NaN, stderr: NaN };
  const mean = usable.reduce((s, v) => s + v.weight * v.mean, 0) / total;
  const variance = usable.reduce((s, v) => s + (v.weight * (Number.isFinite(v.stderr) ? v.stderr : 1)) ** 2, 0);
  return { mean, stderr: Math.sqrt(variance) / total };
}

/** Captured games between two clans, from `a`'s side: games whose two hands are A and B. */
function practice(hands: readonly FieldHand[], a: string, b: string | null) {
  const byGame = new Map<number, FieldHand[]>();
  for (const hand of hands) byGame.set(hand.gameId, [...(byGame.get(hand.gameId) ?? []), hand]);
  let games = 0, score = 0;
  for (const pair of byGame.values()) {
    if (pair.length !== 2) continue;
    for (const [mine, theirs] of [[pair[0], pair[1]], [pair[1], pair[0]]]) {
      if (mine.clan !== a || (b !== null && theirs.clan !== b) || mine.score === null) continue;
      games++;
      score += mine.score;
    }
  }
  return { games, score };
}

/** Solves every cell of the clans' matrix in one batch and summarises it per cell and per clan. */
export async function clanMatrix(
  groups: readonly ClanGroup[],
  options: SolveOptions & { perCell: number; seed: number },
): Promise<ClanMatrixResult> {
  const cells: { a: ClanGroup; b: ClanGroup; pairs: HandPair[] }[] = [];
  for (let i = 0; i < groups.length; i++) {
    for (let j = i + 1; j < groups.length; j++) {
      cells.push({ a: groups[i], b: groups[j], pairs: cellPairs(groups[i], groups[j], options.perCell, options.seed) });
    }
  }
  const { outcomes, cached, solved } = await solvePairs(cells.flatMap((c) => c.pairs), options);
  const allHands = groups.flatMap((g) => g.hands);
  const cellResults: CellResult[] = [];
  const refusals = new Map<string, number>();
  let offset = 0;
  for (const cell of cells) {
    const mine: PairOutcome[] = outcomes.slice(offset, offset += cell.pairs.length);
    for (const o of mine) {
      if (!isRefusedPair(o)) continue;
      const why = describeRefusal(o.reason);
      refusals.set(why, (refusals.get(why) ?? 0) + 1);
    }
    const s = summarize(mine);
    cellResults.push({
      a: cell.a.clan,
      b: cell.b.clan,
      scored: s.scored,
      refused: s.refused,
      mean: s.mean,
      stderr: s.stderr,
      practice: practice(allHands, cell.a.clan, cell.b.clan),
    });
  }
  /** Clan `clan`'s view of a cell: its own mean, negated when it is the cell's B. */
  const seen = (clan: string) =>
    cellResults.filter((c) => c.a === clan || c.b === clan).map((c) => ({
      other: c.a === clan ? c.b : c.a,
      mean: c.a === clan ? c.mean : -c.mean,
      stderr: c.stderr,
      scored: c.scored,
      refused: c.refused,
    }));
  const faced = new Map(groups.map((g) => [g.clan, g.hands.length - g.ownerHands]));
  const clans = groups.map((group): ClanResult => {
    const views = seen(group.clan);
    const equal = weighted(views.map((v) => ({ ...v, weight: 1 })));
    const field = weighted(views.map((v) => ({ ...v, weight: faced.get(v.other) ?? 0 })));
    return {
      clan: group.clan,
      hands: group.hands.length,
      players: group.players,
      ownerHands: group.ownerHands,
      vsClans: equal.mean,
      vsClansErr: equal.stderr,
      vsField: field.mean,
      vsFieldErr: field.stderr,
      scored: views.reduce((s, v) => s + v.scored, 0),
      refused: views.reduce((s, v) => s + v.refused, 0),
      practice: practice(allHands, group.clan, null),
    };
  }).sort((x, y) => (Number.isFinite(y.vsField) ? y.vsField : -9) - (Number.isFinite(x.vsField) ? x.vsField : -9));
  return {
    perCell: options.perCell,
    seed: options.seed,
    night: options.night ?? false,
    clans,
    cells: cellResults,
    refusals: [...refusals].map(([reason, count]) => ({ reason, count })).sort((x, y) => y.count - x.count).slice(0, 20),
    cached,
    solved,
  };
}
