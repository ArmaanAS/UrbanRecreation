// What opponents actually play, per format, from the captured games: the "in practice" half
// of the deck builder's question. Every capture records both four-card hands and the room's
// deck format, so the opposing hands of each format are a sample of its meta - a biased one
// (the owner's own matchmaking, a few weeks) that the numbers carry with them.

/** The parts of a captures/games/<id>.json record this reads. */
export interface GameRecord {
  id: number;
  capturedAt?: string;
  myId?: number;
  mySide?: number | null;
  room?: { idDeckFormat?: number; name?: string } | null;
  players: { side: number; id?: number; hand: { id: number; level: number; clan?: string | null }[] }[];
}

export interface FormatMeta {
  formatId: number;
  /** Opposing hands seen in this format. */
  hands: number;
  from?: string;
  to?: string;
  /** Hands each card was seen in, whatever its level. */
  cards: { id: number; count: number; levels: Record<string, number> }[];
  /** Cards seen per clan, over all hands. */
  clans: { clan: string; count: number }[];
}

/** The owner's side of a capture: `mySide`, or the side whose player id is `myId`. */
function mySide(game: GameRecord, ownerId?: number): number | null {
  if (typeof game.mySide === "number") return game.mySide;
  const id = game.myId ?? ownerId;
  return game.players.find((p) => p.id === id)?.side ?? null;
}

/** The opposing four-card hands of one format, oldest capture first: the field to score against. */
export function formatHands(games: GameRecord[], formatId: number, ownerId?: number): { id: number; level: number }[][] {
  const hands: { at: string; id: number; hand: { id: number; level: number }[] }[] = [];
  for (const game of games) {
    if ((game.room?.idDeckFormat ?? -1) !== formatId) continue;
    const me = mySide(game, ownerId);
    if (me === null) continue;
    for (const player of game.players) {
      if (player.side === me || player.hand.length !== 4) continue;
      hands.push({ at: game.capturedAt ?? "", id: game.id, hand: player.hand.map(({ id, level }) => ({ id, level })) });
    }
  }
  return hands.sort((x, y) => x.at.localeCompare(y.at) || x.id - y.id).map((h) => h.hand);
}

export function formatMeta(games: GameRecord[], formatId: number, ownerId?: number): FormatMeta {
  const cards = new Map<number, { count: number; levels: Record<string, number> }>();
  const clans = new Map<string, number>();
  let hands = 0;
  let from: string | undefined, to: string | undefined;
  for (const game of games) {
    if ((game.room?.idDeckFormat ?? -1) !== formatId) continue;
    const me = mySide(game, ownerId);
    if (me === null) continue;
    for (const player of game.players) {
      if (player.side === me || player.hand.length === 0) continue;
      hands++;
      if (game.capturedAt) {
        if (!from || game.capturedAt < from) from = game.capturedAt;
        if (!to || game.capturedAt > to) to = game.capturedAt;
      }
      for (const card of player.hand) {
        const entry = cards.get(card.id) ?? { count: 0, levels: {} };
        entry.count++;
        entry.levels[card.level] = (entry.levels[card.level] ?? 0) + 1;
        cards.set(card.id, entry);
        if (card.clan) clans.set(card.clan, (clans.get(card.clan) ?? 0) + 1);
      }
    }
  }
  return {
    formatId,
    hands,
    ...(from ? { from, to } : {}),
    cards: [...cards.entries()].map(([id, v]) => ({ id, ...v })).sort((a, b) => b.count - a.count || a.id - b.id),
    clans: [...clans.entries()].map(([clan, count]) => ({ clan, count })).sort((a, b) => b.count - a.count),
  };
}
