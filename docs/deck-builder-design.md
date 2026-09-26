# Deck builder on the Urban Rivals site: design proposal (draft)

Draft for the owner to react to, 2026-09-26. **Phases 0 and 1 are built** (read-only; see
"Status" below); everything from phase 2 on is still a proposal. It builds on
`docs/deck-building.md` (the backlog write-up) and three read-only investigations of the same
day: `docs/site-api.md` (what the site exposes, from captured traffic), how the userscript and
log server work, and the solver's coverage and solve costs (the last two are summarised here and
in `docs/deck-building.md`).

## Status (2026-09-26)

Built the same day, autonomously, after the owner asked for the work to continue; nothing
writes to the account:
- **Phase 0**: the log server refuses every origin but the site's and local tools
  (`140e038`), serves the userscript at http://localhost:8787/ur-logger.user.js for one-click
  updates, and warns when the browser's copy is out of date.
- **Phase 1**: `src/decks/` (a port of the site's own validator, deck reports, the deck
  service), `scripts/DeckCapture.ts` (passive capture in the log server, raw-log stand-ins
  for the catalog pages) and `deno task deck-data` (`352bbfc`), then the Collection Pro panel
  in userscript 0.9.0 (`f821484`).

Where it differs from the plan below:
- The validator is a port of the site's client-side `DeckFormat.parseDeck`, not a
  reimplementation from the criteria descriptions, and its oracle is better than the
  deck-name guess: the game client's `collections.decks {deckFormatID}` returns only the decks
  the **server** calls legal in that format. Ours agrees on all 76 deck-format pairs.
- The panel reads the deck being edited from the page's own deck list (every card link
  carries id, level and edition), not from the last `loaddeck`, so it follows unsaved edits.
- The panel reaches the deck service through the log server's `/decks/` rather than
  `127.0.0.1:8788` directly: Edge prompts separately for each local address a site calls,
  and an unanswered prompt froze the tab.
- The deck service has `/api/deck/:id` and `/api/card/:id` rather than per-level card routes,
  plus `POST /api/report` for any list of cards.

A first cut of **phase 3** followed: Deck Lab (`src/decks/ui/`), served by the deck service at
http://127.0.0.1:8788. It browses all 2,498 cards with the owner's copies (filters for clan,
rarity, owned, "legal in this format", text; level shown as the highest owned, max or min;
sorting), drafts a deck from them or from a saved deck (levels up and down, remove), and shows
the same live report as the panel plus the difference from the saved deck it started from.
Drafts are kept in the browser's local storage and can be exported as JSON. Nothing in it
talks to the site.

Then the **phase 5 engine**, command line only for now: `urban-recreation-matchup`
(`rust/src/advisor/matchup.rs`), `src/decks/Matchup.ts`, `deno task matchup` and
`deno task card-coverage`. Measured: T1 Rescue against T1 Riots, N = 40, took 36 s on 6
threads (78 solves at about 2.7 s each) and gave 49.5% ± 2.6; a cached rerun takes 0.6 s.
Coverage at max level: 54.2% of cards exact by day, 55.2% by night. Two things differ from the
plan: hands are solved in sorted `(id, level)` order, so Symmetry/Asymmetry see one fixed
arrangement rather than a random deal, and a draw's value is the advisor's top-ranked move's,
which the rounded-percent ranking can put up to half a point below the best raw average.

Deck Lab then got both (same day): every card carries a badge when the solver cannot score it
at that level and time of day, with the reason (from `data/card_coverage.json`, flagged when
the file no longer matches the engine or card data), a "Solver can score" filter, a line
under the draft naming its unscorable cards, and a **Score** button. It scores the draft
against the captured opposing hands of the chosen format (the "meta" of the plan, taken as
the real hands rather than assembled decks) or against a saved deck, in the deck service
(`POST /api/matchup`, polled; one job at a time, a new one or Stop kills the binary, finished
solves stay cached). Its seed is fixed, so two drafts meet the same opposing hands. T1 Rescue
against 20 captured Tourney opponents: 45.5% ± 3.9, 15 of 20 pairs scored (two refused for
Leaders, two for Anita's Courage Life conversion, one for an After-clan ability), 10.8 s.

To use it: update the userscript from http://localhost:8787/ur-logger.user.js, run
`deno task decks` beside the log server, open Collection Pro and click "UR Lab", or open
http://127.0.0.1:8788 for Deck Lab.

## What changed since the backlog write-up

Three of the backlog's "missing" items are already answered by traffic the logger captured while
you had Collection Pro open.

- **Format rules exist in machine-readable form.** `POST /ajax/collection/ action=deckformatsdata`
  returns every format with typed criteria. On 2026-09-23:

  | Format (id) | Room / battle rule | Rules |
  | --- | --- | --- |
  | Tourney (54363) | 16193 / 10 | at least 8 cards, **32 stars max**, 218 banned, 133 banned at max level, no duplicates |
  | EFC (1) | 5 / 3 | at least 8, **25 stars max**, 4 weekly vote bans, 198 banned at max level, no level-1 cards, at most one level-5 card, staff ELO bans excluded, no duplicates |
  | Free Fight (57215) | 13044 / 1 | at least 8, no duplicates |
  | Survivor (55009) | 1202394 / 4 | at least 10, no duplicates |
  | none (0) | Training, Dojo | none |

  "Stars" is settled: the site's own rule text says "the sum of the card levels in your Deck must
  not exceed 32". No format limits Leaders, clans or maximum deck size. The ban lists move every
  week, so they are fetched, never hard-coded. The solver investigation guessed a 25-star Tourney
  cap from your two Tourney decks. The real cap is 32. Your decks sit at 25, probably because
  Tourney awards points for having fewer stars than your opponent (the room description says so).
- **The collection is one call away.** `action=collectiondata` (five pages of 500) returns the
  whole catalog, 2,498 cards, each with per-level power, damage, ability (with ability id), bonus,
  night variants, picture URLs, every ban flag and `collectionData`: your owned copies per level
  and per edition. This is more complete than the local `site_characters.jsonl`, which holds only
  2,030 characters. That dump is partial, which is why its ban counts (168) disagree with the
  format list (218).
- **Decks can be read and written with one endpoint.** `loaddeck` reads one deck. `savedeck`
  (`id`, `name`, `set_current`, `characters[i][id|level|state]`) overwrites a whole deck, or
  creates one with `id=0` (the old log shows it returning a fresh id). The game client's
  `collections.decks` lists all 19 of your decks. The account limit is 21, so two slots are free.

All of it is cookie-authenticated, same-origin form posts from the classic PHP page. A panel on
Collection Pro can call it directly without touching the game client's access token.

## 1. Architecture

### Options considered

**A. Userscript panel on Collection Pro only.** A panel injected into `/collection/pro/` shows card
details, legality and (later) solver advice next to the site's own editor.
- For: it lives where you already build decks, and it can ship this week.
- Against: it is boxed in by the site's page. No room for a collection grid, filters or matchup
  matrices. The userscript has to be reinstalled for every UI change, and heavy logic in a
  userscript is hard to test.

**B. Our own web UI on a local server, with the userscript only as a bridge.** A local page with
a full collection browser, a draft deck editor, validation and solver results. The userscript only
refreshes the data and applies a finished deck.
- For: the gold-star experience, tested TypeScript, direct access to the engine and the Rust
  worker.
- Against: it is the most work before you see anything, and you still need the site open to read
  the collection or apply a deck.

**C. Both, in stages, over one shared back end.** Recommended.

### Recommended shape

```
Edge, urban-rivals.com                            this machine
 ur-logger.user.js (capture hooks, unchanged)
   + passive deck capture   --POST /log------->   log_server.ts :8787   capture only, never CPU-heavy
   + Collection Pro panel                           writes data/deck_formats.json,
     (thin renderer, Shadow DOM)                    data/my_collection.json, data/my_decks.json
        |  GET /api/... (JSON)
        v
                                                   deck service :8788   (`deno task decks`, new)
                                                     validator, card details, coverage,
                                                     drafts, proposals, solver jobs (subprocesses)
                                                     later: serves the own UI at /
```

The design rules:

1. **The userscript stays a thin pipe.** This is the existing philosophy: all interpretation lives
   on the local side so the script is rarely reinstalled. The panel renders JSON that the deck
   service computes (validation verdicts, card details, coverage badges). The validator is written
   once, in TypeScript, and both the panel and the own UI use it.
2. **The capture server never gets heavier.** `Advisor.ts` explains why: it must keep up with the
   game's polling. Deck logic goes in a separate process on :8788. Solving goes in subprocesses of
   that process.
3. **Reading costs the site nothing extra by default.** The server recognises `collectiondata`,
   `deckformatsdata`, `loaddeck`, `savedeck`, `collections.decks` and `rooms.list` responses when
   the page loads them, the same way it recognises battle traffic. Simply opening Collection Pro
   refreshes the files. An explicit `__ur.dumpCollection()` button covers the case where the page
   did not load everything.
4. **Writes go through one guarded path**, used by the panel and the own UI alike (section 4).
5. **The panel and the own UI share one data model**, so the own UI replaces the panel gradually
   rather than forking it.

Why not iframe the own UI into the site page? Loopback `http` from an `https` page generally works
in Chromium (the logger already POSTs to localhost), but embedding a frame raises framing and
mixed-content questions that JSON fetches do not. The live checklist covers it. It can be
revisited once the own UI exists.

## 2. Data model

Everything personal is gitignored. Everything public and derived-from-public is either committed
or regenerable.

| File | Content | Source | Git |
| --- | --- | --- | --- |
| `data/deck_formats.json` | `{fetchedAt, formats:[{id, name, isOfficial, criteria:[{name, description, value}]}], rooms:[{roomId, name, idDeckFormat, idBattleRule, minLevel}]}`, verbatim criteria plus the room mapping from `rooms.list` | `deckformatsdata`, `rooms.list` | commit: public, and the diff shows weekly ban changes |
| `data/site_cards.json` | public card fields per id: name, clan, rarity, kind, level range, per-level power/damage/ability `{id, typeID, unlockLevel, description}`, night ability, bonus, night bonus, ban flags, `efc_bonus_low/high`, picture URL, release date | card part of `collectiondata` | gitignored (large). It can also become the complete source for `deno task cards`. |
| `data/my_collection.json` | `{fetchedAt, cards:{<id>:{<level>:{<state>:count}}}}` with states `""`, `p`, `s`, `m1..m3`, `rp`, `i` | `collectionData` of `collectiondata` (or `collections.get`) | gitignored |
| `data/my_decks.json` | `{fetchedAt, maxDecks, decks:[{id, name, isCurrent, characters:[{id, level, state}]}]}` | `collections.decks`, `loaddeck`, `savedeck` | gitignored |
| `data/deck_history.jsonl` | one line per applied write: `{t, deckId, before, after, siteResponse}` | the apply path | gitignored. This is the undo log. |
| `data/deck_drafts.json` | your drafts in the own UI: `{draftId, name, targetFormat, characters, notes}` | own UI | gitignored |
| `data/card_coverage.json` | per `(id, level, day/night)`: `exact`, `bonus_refused`, `bonus_untested`, `refused`, `leader` or `missing`, each refusal classed as uncaptured text, uncaptured id or not executable, plus the binary's provenance | `deno task card-coverage` over the Rust probe (the TS parser column is not built) | gitignored: regenerate |
| `cache/matchups/v1-<provenance hash>.jsonl` | one `{k, r}` line per solve: key = night, life, pillz and both sorted `(id, level)` hands, first mover's first; result = `{value, worst, best_move, ko_share, koed_share, ms}` or `{refused}`; a `.provenance.json` beside it says what the hash stands for | `src/decks/Matchup.ts` | gitignored |
| `data/analysis/*.json` | clan matrices and deck evaluations with N, standard error, prior and fingerprints | solver runner | decide per result |

Derived views the service computes on request (not stored):

- **Deck legality per format**: every criterion as pass, fail or unknown, with the site's own
  description text. An unknown criterion name means "cannot validate", never pass. Owned checks
  are exact: level and edition must be owned, with no silent edition substitution.
- **Deck summary**: stars against the cap, clan counts, and whether the clan bonus is guaranteed
  in every hand or only in some fraction of the 70 hands. Leader and Oculus warnings. Coverage
  mix (for example "8/8 exact", or "6 exact, 2 TS-only").
- **Card detail**: everything the site shows badly, at the chosen level. Ability and bonus text
  (day and night) with `[clan:N]` tags rendered via `GameRenderer`, the level where the ability
  unlocks, ban status per format, owned levels and editions, the market's minimum price and the
  coverage badge.
- **Meta per format**: opposing card-level and clan frequencies from `captures/games/*.json`
  (Tourney 268 games, EFC 67).

Editions (`state`) never change stats, so the engine collapses them. The site, however, needs the
exact state to save a deck, so decks keep it.

## 3. Phased plan

### Phase 0: hardening (small, do first, separate commit)

- `POST /log` accepts only `Origin: https://www.urban-rivals.com`, or no Origin for local tools.
  Stop answering with `Access-Control-Allow-Origin: *`; reflect the allowed origin instead. Today
  any web page can forge a `clans` record that overwrites `data/site_clans.json`, or truncate
  `site_characters.jsonl`. This matters more once personal data endpoints exist.
- The userscript reports its `@version` in the `page` record. The server prints "userscript 0.7
  is outdated" instead of silently missing features.
- Serve `GET /ur-logger.user.js` from the log server and add `@updateURL`/`@downloadURL`. New
  versions then install with one click in Edge. It needs `--allow-read=ur-logger.user.js` in
  both `deno.json`'s `log` task and `MANAGED_LOG_ARGS` in `Advisor.ts`.

### Phase 1: read-only deck panel (the increment to try this week)

Needs: phase 0, plus the live checks marked (P1) in section 5.

1. **Seed from the existing raw log.** A streaming extractor (`TextLineStream`, reads only
   response bodies and action/call names) writes the files in section 2 from what is already in
   `ur_log.jsonl`. You see results before touching the userscript.
2. **Passive capture in `log_server.ts`**: recognise the six responses and write the same files.
   Trim before appending to `ur_log.jsonl`. Every Collection Pro visit otherwise adds about 13 MB
   of catalog to the raw log. Only personal and format fields need keeping, since card text is
   written to `site_cards.json` once.
3. **Validator module plus tests** pinned on the captured `deckformatsdata`. A good first oracle:
   each of your 19 saved decks should pass the format its name implies ("T1 ..." Tourney,
   "EFC ..." EFC, "FF ..." Free Fight). And 268 captured Tourney and 67 EFC opposing hands must
   not violate their format's bans.
4. **Deck service on :8788** with `GET /api/formats`, `/api/decks`, `/api/deck/:id/report`,
   `/api/card/:id/:level`.
5. **Userscript 0.8**: on `/collection/pro/` only, a collapsible side panel in a Shadow DOM. It
   shows the deck the page last loaded (seen through the XHR hook), its legality for every format,
   stars against caps, clan counts, and one row per card with full details. No write calls exist
   in 0.8.

Delivers: open Collection Pro and see, next to the site's editor, exactly why a deck is or isn't
legal and every card's full text, with nothing sent to the site that the page didn't send itself.

### Phase 2: coverage and details everywhere

- `deno task card-coverage` writes `card_coverage.json` from the Rust probe (built): statuses
  exact / bonus_refused / bonus_untested / refused / leader / missing, and every refusal classed
  as uncaptured text, uncaptured id or not executable. The TS parser's "compiles to nothing"
  column is not built.
- The panel and the service show the badge on every card, and a deck-level coverage line
  (done in Deck Lab; the Collection Pro panel does not show it yet).
- If the site DOM exposes card ids (live check), hovering a card in the site's own grid shows the
  panel's detail card for it. This targets "doesn't display all the details nicely" directly.

### Phase 3: the own UI (gold star, read-only)

Served by the deck service at `http://127.0.0.1:8788/`.
- A collection grid over all 2,498 cards with filters: clan, rarity, owned or not, owned level,
  stars, legal in a chosen format, coverage status, ability keyword, day/night.
- A draft editor with live validation, star budget, clan bonus consistency and "cost to complete"
  from market minimum prices.
- Your 19 decks side by side, with a diff between any two.
- Drafts stay local. Nothing is written to the site in this phase.

The own UI does not depend on phases 5-7. It can go in parallel with them.

### Phase 4: guarded apply (userscript 0.9)

Needs: the live checks marked (P4), and your agreement on a dedicated deck slot.
- The panel shows a pending proposal (a draft you marked "send to site" in the own UI). An
  explicit click saves it via `savedeck` under the rules in section 4. The result is verified by
  re-loading the deck and logged to `deck_history.jsonl`.
- "Make current" is a second, separate button.

### Phase 5: deck versus deck evaluation

Needs: a batch hand-pair runner on the Rust worker (many draws, parallel across pairs, results
cached by hand pair and fingerprints), plus coverage from phase 2.
- **Deck versus deck**: stratified by first mover (always both), N = 100 hand pairs gives about
  ±1.5 percentage points. Measured on real decks: about 2.7 s per exact solve (0.7-5.7 s), so
  N = 100 is about 90 s and N = 400 about 6 min on 6 cores (the first estimate, 1.2-1.9 s,
  came from easier draws).
- **Comparisons use common random numbers**: the same opponent hands and first movers for both
  candidates. A one-card swap reuses half its hands from cache.
- **Delivers**: your decks against each other and against "meta decks" assembled from the
  captures, with the worst draws listed. Shown in the own UI and as a panel line ("vs meta: 54%
  ±1.5").
- **A draw the Rust engine refuses** is reported as refused, not silently scored. A TS fallback
  can come later, only after a parity check on a shared subset.

### Phase 6: clan matrix for one format (Tourney first)

**First cut built (2026-09-26), from captured hands rather than representative decks.**
`src/decks/ClanMatrix.ts`, `deno task clan-matrix [--night]`, shown by Deck Lab's Clans button.
A hand belongs to a clan when three or four of its cards do; each clan pair gets 16 hand pairs
drawn from the two clans' captured Tourney hands (both sides of every game, so most hands of the
owner's own clans are the owner's), solved with both first movers. "vs clans" weights every other
clan equally; "vs field" weights them by how often the owner's opponents play them. Next to it,
"in practice": how those hands did in the captured games themselves. Tourney, 21 clans with 7+
hands, 210 clan pairs: 6,266 solves in 21 minutes by day, the same again by night. 38% of hand
pairs by day (35% by night) are refused and left out, led by Leaders (124), `Day: -2 Opp Pillz.
Min 0` (84, never captured), the `Tune Out` bonus (69), `Cancel Opp. Pillz & Life Modif.` (56)
and `Dope 3, Max. 4` (51), so the "Solved" column matters. By day, vs field: Roots 57%, Raptors
55.5%, Zenith 55%, Riots 54.5% at the top; the owner's clans Ulu Watu 50.3%, Paradox 49.3%, Hive
46.6%, Rescue 43.3%; Cosmohnuts and All Stars 42% at the bottom (about ±1.5 each, Zenith, Tolvack
and Oblivion ±3-5). The most one-sided pairs: Raptors over Rescue 81%, Roots over Raptors 81%,
Nightmare over Rescue 72-74%. Deck Lab's Score against the field also breaks the draft's result
down by the opposing hand's clan (T1 Rescue, 60 field hands: 24% against Raptors and Montana).

The plan as first written, for when representative decks exist:

Needs: phase 5, and a representative deck per clan: the best legal mono-clan deck under 32 stars
from exactly covered cards, chosen by the heuristic, plus a few obvious dual-clan decks.
- 35 × 34 / 2 = 595 pairs at N = 40 (±2.4 pp) is 47,600 solves: about 6 h on 6 cores at the
  measured 2.7 s a solve, so an overnight run. N = 100 (±1.5 pp) is about 15 h, a weekend,
  unless the solve gets cheaper or the matrix is cut to the clans that matter.
- **Delivers**: the matrix, its rock-paper-scissors cycles, and next to it the practice numbers
  from the captures as a sanity check.
- **Caveats printed alongside**: conservative policy rather than equilibrium; the shared opening
  prior, with a uniform-prior sensitivity run; coverage bias toward old cards and well-covered
  clans; GhosTown only scorable at night; Leaders unscorable.

### Phase 7: deck search from your collection

**One slot at a time, built (2026-09-26)**: `src/decks/Swap.ts`, Deck Lab's ⇄ on a draft row.
It tries every owned card of the slot's clan (or of the deck's clans) that is not in the deck, at
its highest owned level that the solver can score and that keeps a legal deck legal, 40 at most
(strongest power + damage first), and scores each variant against the same opposing hands as the
draft. A deck's own hands depend only on its size and the seed, so a variant shares every hand
that does not draw the slot: those pairs cost nothing and differ by exactly zero, and the ranking
uses the paired difference, which keeps the error small at N = 30. T1 Rescue's Wesley L3 against
30 captured Tourney opponents: 58 owned Rescue cards in 296 s (1,508 solves), Reeve L5 +4.8 ± 1.6
points, Sledg Cr L5 +4.2, Bulma L5 +3.9, Ghoub L4 +3.1; most others within a point of zero. A
candidate refused in every hand that holds it shows "?", not zero. "Use" puts it in the local
draft; nothing is written to the site.

The plan as first written, for a whole-deck search:

Needs: phases 5-6, the cache, and a surrogate. The position heuristic costs 5.4 ms and correlates
r ≈ 0.8 with the exact value within each first-mover stratum. Its first-mover bias has the
opposite sign, so it needs calibrating per perspective before it is used.
- Enumerate legal candidates from owned cards, plus "buy" candidates if you want them. Score with
  the calibrated surrogate: deck value is exactly the mean of its 70 hands' values, so search
  reduces to maximising an average of cached hand scores against a meta. Confirm a shortlist of
  about 5 with N ≥ 400 exact solves per matchup.
- **Delivers**: a shortlist with reasons. For each deck: matchup rows, worst draws, stars used (and
  Tourney star points), each card's contribution, and cost to acquire. You pick.

Exact local search alone is out of reach (4-6 h per step on 6 cores). That is why the surrogate
and cache come first.

## 4. Safety rules for anything that touches your account

1. **Only an explicit click writes.** No timer, no server push, no auto-apply after a solve. The
   deck service may offer a proposal; only your click in the panel turns it into a request.
2. **A hard-coded allowlist in the userscript.** The write path may call `loaddeck` and
   `savedeck`, and `setcurrentdeck` only from its own separate button. Never `evolve`, market
   `sell`, `purchase`, bank sales, or any state-changing `/api/private/v2/` call. Market data is
   read-only price display. The allowlist lives in the script, not in anything the server sends.
3. **Dry run first.** Validate against the freshly fetched formats (an unknown criterion refuses),
   against your collection (level and edition owned), and against duplicates. The Apply button
   does not appear until the dry run passes.
4. **Show a diff.** `loaddeck` the target first. List removed, added and level-changed cards and
   the star change. The button label names the deck being overwritten.
5. **Dedicated slot.** Save into a deck you designate (for example "UR-Lab"; you have 2 free
   slots), or create a new one after the live check confirms `id=0`. Overwriting a named mode deck
   needs a typed confirmation. `set_current=false` always, unless you press "make current".
6. **Verify and keep an undo.** Re-load after saving and compare. Append the before and after
   to `deck_history.jsonl`, so any save can be reverted by re-saving the old list.
7. **The site's validation is the backstop, not the plan.**
8. **Read traffic stays modest.** Passive capture by default. An explicit dump fetches at most the
   five catalog pages plus formats and decks, on click, never on a timer.
9. **No secrets in files.** Deck files hold only card ids, levels, states, deck names and ids.
   Nothing from `auth.*`, `general.initPlayer` (it contains your email) or request headers is
   written anywhere new. Personal files are gitignored.

## 5. Live checklist for a browser session on https://www.urban-rivals.com/collection/pro/

Everything is read-only unless marked **(owner, scratch deck)**. Record header *names* only, never
values; cookies, tokens and CSRF values stay out of notes. (P1) = needed before phase 1,
(P4) = before apply.

**Environment**
- [ ] (P1) Tampermonkey in Edge: "Allow User Scripts" is on for the extension. Exactly one copy of
      `ur-logger.user.js` is enabled, and it is v0.7. `__ur` exists in the console, and
      `deno task log` prints the `page` record for `/collection/pro/`.
- [ ] (P1) No "local network access" prompt or blocked request to `127.0.0.1:8787` or `localhost`
      in the console. If there is a prompt, note what Edge offers.
- [ ] (P1) Response headers of the `/collection/pro/` document: any `Content-Security-Policy` or
      `X-Frame-Options`, and any `<meta http-equiv>` CSP in the source.

**How the page works**
- [ ] (P1) The request headers of one `/ajax/collection/` XHR, names only: `X-Requested-With`? A
      CSRF header? Any token field in the form body besides `action`?
- [ ] (P1) Page source or inline JS: where the `collectiondata` page count and the current deck id
      passed to `loaddeck` come from (a global variable? a data attribute?).
- [ ] (P1) Where the page keeps the deck being edited before it is saved: a JS global, a
      framework store, or only the DOM. Can the panel read unsaved edits?
- [ ] (P2) DOM: do card tiles carry id and level (`data-*` attributes, image URLs)? Where does a
      side panel fit without covering the editor?
- [ ] (P1) From the console, one read-only `fetch('/ajax/collection/', {method:'POST',
      credentials:'same-origin', ...body:'action=loaddeck&id=<current id>'})`. Does it work with
      only the cookie? With `X-Requested-With`?
- [ ] Does Collection Pro have a format selector, and does it show legality itself? What does it
      show for a banned card?

**Decks**
- [ ] (P4) `/collection/decks/list.php`: inspect (do not click) the delete control's handler or
      form. Which action or URL would it call? Same for "new deck".
- [ ] (P4) Watch the Network tab while the site's own "new deck" flow saves. Is it `savedeck` with
      `id=0`?
- [ ] (P4) **(owner, scratch deck)** In a spare slot named "UR-Lab", save a deliberately illegal
      Tourney deck through the site's own UI (for example 33 stars). Does `savedeck` refuse it,
      and with what response shape? Does anything refuse it before a room join?
- [ ] Is `savedeck` format-less everywhere, or does any request carry a format id?

**Data meanings**
- [ ] Tooltips or legend for the `rp` and `i` editions, and for `efc_bonus_low`/`efc_bonus_high`
      (the EFC room description or the Collection Pro filters may explain them).
- [ ] The EFC "staff ELO bans" list: is it shown anywhere (EFC rules page or room description)?
      Compare with the 267 `efc_banned` cards.
- [ ] Does the page mark copies you have on sale? Can a copy on sale be put in a deck? (The market
      "my sales" section's request, observed only.)
- [ ] Do the ability objects anywhere (Collection Pro, `getcharacter`, character pages) carry
      structured `abilityData` parameters beyond id, type id and text? This decides whether
      uncaptured abilities can reach the Rust registry.

**Nice to know**
- [ ] `/presets/?id=...` and any preset search page: which XHR loads them (a possible meta source).
- [ ] The site's own `autodeck-generate.php` output for one room and clan (a baseline to beat).
- [ ] On a `/game/play/webgl/` tab: the name of the header carrying the private-API access token.
      Name only.

## 6. Open questions for you

1. Tourney first? And is the goal in Tourney pure win chance, or should the star count (points
   for fewer stars) be part of the score, given your decks sit at 25 of a 32 cap?
2. Build only from your collection, or also suggest purchases (read-only prices)?
3. Consistency or peak: rank decks by average, by worst draws, or show both?
4. May the tool save only into a dedicated deck ("UR-Lab"), with overwriting a named deck needing a
   typed confirmation?
5. Agree with the order: read-only panel this week, own UI next, apply after? Or skip straight to
   the own UI?
6. Commit `data/deck_formats.json` (public, shows weekly ban changes) and gitignore your
   collection and decks?
7. Should `deno task cards` switch to the Collection Pro catalog? It is complete (2,498 cards,
   including 2714), while the current `site_characters.jsonl` dump is partial (2,030).
8. Uncaptured abilities: keep the strict Rust registry battle-capture-only, or admit site catalog
   text as a separate "unverified" tier? About 30% of max-level cards are unscorable only because
   no capture has shown their ability.
9. Is the conservative model acceptable for a first clan matrix, with its caveats printed? Or
   should a mixed-strategy check of ranking stability come first?
10. Day or night by default? GhosTown's day bonus is not yet in the registry, so it can only be
    scored at night.
11. Land the `/log` Origin hardening now as its own commit, ahead of any deck work?
