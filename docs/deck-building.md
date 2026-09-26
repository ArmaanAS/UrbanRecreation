# Deck building for a game mode (backlog)

Status: **not started.** The owner raised this on 2026-09-26 as the next bigger thing after
the solver work. It is on the backlog to think about, not a committed plan. Everything below
is a starting point for that thinking, and every rule marked *to confirm* needs the owner's
word or a capture before anything is built on it.

## The problem

The owner has to build a deck for a specific mode before any of the advisor's work matters,
and building one is hard. The rules constrain it, the combinations are many, and the choices
interact:

- **Mode rules** (*to confirm per mode*): a maximum total of stars, cards banned in that mode,
  deck size, and how many copies or Leaders a deck may hold.
- **Clan structure**: single-clan decks guarantee the clan bonus; dual-clan decks trade that
  for coverage; Oculus joins a clan only when the drawn hand makes it the odd one out; two
  Leaders in a hand cancel each other's abilities (`Game` constructor).
- **The draw**: a battle deals four cards from the deck, so a deck is really a distribution
  over hands. Consistency (how bad the worst draws are) matters as well as the average.
- **The collection**: a deck can only use cards the owner owns, at the levels they own them,
  unless the question is "what should I buy".

The owner's hope is that the solver can help decide, for example by ranking clans against each
other: which clan is weakest against which, both in theory and in practice.

## What already exists

- **A hand-versus-hand evaluator.** Given two four-card hands, the Rust advisor now solves a
  whole match from round one exactly, single-threaded, in about 0.2-2 s (`aa1ea2f`), and the
  TypeScript engine does the same more slowly. That value - who wins under the conservative
  information-aware policy - is the natural primitive for everything below.
- **Card data for every card and level** (`data/data.json`), including stars, clan, power,
  damage and ability text, refreshed from the site's own card DB.
- **A corpus of real games.** 383 captures; in Tourney (battle rule 10, 268 of them) the
  opponents' hands show 722 distinct card-and-level pairs, a real if partial picture of what
  people play. Each capture records the room and its `idDeckFormat` (Tourney 54363, EFC 1,
  Free Fight 57215, Survivor 55009), but not what that format allows.

## What is missing

1. **The format rules themselves.** `idDeckFormat` is an id without a definition. The rules
   probably come from the site (deck-builder or room endpoints); the capture userscript
   could record them, or the owner can write them down per mode.
2. **The owner's collection**: which cards, at which levels. Probably a site endpoint the
   userscript can capture, like `__ur.dumpCharacters()` does for the card DB.
3. **Engine coverage per card.** The Rust engine is fail-closed: it refuses any draw holding
   an ability it cannot model exactly (302 of 383 captured draws are fully supported). A deck
   evaluator has to know which candidate cards it can score exactly, fall back to the
   TypeScript engine (broader, slower, less strictly checked) for the rest, and say which
   numbers rest on which.

## Candidate approach, in layers

1. **Hand versus hand** (have it): exact match value for two hands and a first mover.
2. **Deck versus deck**: an eight-card deck has 70 four-card hands, so a full deck-versus-deck
   value is 70 x 70 hand pairs x 2 first movers = 9,800 solves - over an hour single-threaded
   per pair. Needs sampling (with a stated error), a cache of hand-pair results, or a cheaper
   learned surrogate trained on exact solves.
3. **Clan versus clan, in theory**: represent each clan by one or more strong legal decks
   (best cards under the star cap, single-clan and the obvious dual-clan pairings) and fill a
   clan-by-clan matchup matrix. That answers "which clan is weakest against which" directly,
   and exposes rock-paper-scissors cycles.
4. **Clan versus clan, in practice**: win rates by clan matchup from the captures. Sparse and
   biased (most games are the owner's own few decks), so it is a sanity check on layer 3,
   not a replacement for it.
5. **Deck search**: generate legal candidate decks (star cap, bans, clan structure, owned
   cards) and optimise expected value against a *meta* - a distribution over opponent decks,
   seeded from what the captures show people playing - with local search or beam search on
   the cheap evaluator, then confirm the shortlist with exact solves.
6. **The owner decides.** The output is a shortlist with reasons: matchup rows, the worst
   draws, star efficiency, what each card contributes and what it costs to acquire. Picking
   the deck stays a human call.

## Things to be careful about

- **The solver's "theory" is a conservative model, not an equilibrium.** It assumes an
  opponent who answers our hidden bets as badly for us as possible. It is the right model for
  live advice, and it is biased for comparing decks. A mixed-strategy model would be the
  principled "in theory" number; measure how much the ranking changes before trusting it.
- **Real opponents are not the model.** The captured opening prior (390 opponent round-one
  plays) is one small window onto how people actually bet.
- **Coverage bias.** If the evaluator can only score some cards exactly, the search will
  drift toward those cards. The report has to show that, not hide it.

## Open questions for the owner

- Which mode first? Tourney is most of the corpus.
- The exact rules for that mode: star cap, bans, deck size, Leader and duplicate rules.
- Build from the current collection, or also suggest purchases?
- How to weigh consistency against peak strength: best average, or best worst-case draw?

## Possible first steps, when this starts

1. Capture the format rules and the owner's collection (userscript additions, like the card
   dump).
2. A batch hand-versus-hand runner on top of the Rust advisor (many draws, all cores, results
   cached on disk), plus a per-card coverage report.
3. A first clan matchup matrix for one mode, with its caveats written next to it, to see
   whether the numbers look like the game the owner knows.
