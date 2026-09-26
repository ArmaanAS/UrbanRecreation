# Urban Rivals site interfaces for a deck builder (from captured traffic)

Mapped on 2026-09-26 from traffic the userscript had already captured, streamed with throwaway
Python scripts (not kept). Sources, both gitignored raw logs that contain tokens:

- `ur_log.jsonl` in the monorepo worktree ("new", 109 MB,
  t = 1789636619473 .. 1790199555981, i.e. 2026-09-17 .. 2026-09-23)
- `ur_log.jsonl` in the old primary checkout ("old", 667 MB, 2026-09-09 .. 2026-09-15;
  some very large bodies there were replaced by `PruneLog.ts` with `[dropped by PruneLog: N chars]`)

Secrets: none are reproduced here. The log does not record request headers at all (see section 6),
so the access token never appears in request records; it does appear in `auth.exchangeToken` /
`auth.refresh` responses and must be redacted by anything that re-publishes those.

## 0. The two transports

The site has two separate back ends, and a deck builder needs both.

| Transport | Used by | Auth | Encoding |
| --- | --- | --- | --- |
| `POST https://www.urban-rivals.com/ajax/<area>/` (XHR, jQuery-style) | The classic web pages: `/collection/pro/`, `/market/`, character pages | Session cookie (same origin) | `application/x-www-form-urlencoded`, method is the `action=` field; PHP array syntax for lists (`characters[0][id]=...`) |
| `POST https://www.urban-rivals.com/api/private/v2/` (fetch) | The Unity WebGL game client (`/game/play/webgl/`) | Bearer-style access token obtained via `auth.exchangeToken` (header not logged, see section 6) | Body is `requests=` + URL-encoded JSON array `[{"call":"<area>.<method>","params":{...}}]`; response is a JSON object keyed by the call name: `{"<call>": {"data": ...}}`. Every captured request carried exactly one call (no batching seen). Errors come back as `{"errors":[{"id":4,"message":"Expired or invalid access token."}],"error":{...}}` |

So the private-API "method name" is not a URL path or header: it is the `call` field inside the
URL-encoded `requests=` form value. 249 early records (old log only) have body `[binary]` or `null`
(the Unity client sent a byte array before the userscript learnt to decode it); for those the method
name is only recoverable from the response's top-level key.

All `/ajax/*` traffic in both logs is XHR; all `/api/private/v2/` traffic is fetch.

## 1. The collection

### 1a. Collection Pro page: `/ajax/collection/ action=collectiondata`

Opening `/collection/pro/` fires, in this order: 5 parallel `collectiondata` pages, then
`deckformatsdata`, then `loaddeck` for the current deck id (seen on every one of ~20 visits).

```
POST /ajax/collection/
action=collectiondata&page=0&nbPerPage=500        (pages 0..4 fetched concurrently)
```

Response: a bare JSON array, 500 rows per page (last page 498 on 2026-09-23 -> 2498 characters).
There is no `hasNextPage`/total in the response; the page must learn the page count elsewhere
(probably the HTML; not captured). The rows are **the whole card catalog**, not only owned cards:
ownership is the `collectionData` sub-object (2099 of 2498 characters owned by the account, 3363
copies). Row keys (all 36, identical in every row):

`id, name, clan_id, clan_name, level_min, level_max, rarity ("c"/"u"/"r"/"l"/"cr"), rarity_value (0..4),
kind ("normal"|"collector"|"oculus"|"legend"|"leader"), is_noel, is_miss, is_fantasy, is_titan,
is_ultra, is_clan_leader, is_meteora, id_faction, efc_banned, efc_max_evo_banned, efc_temp_banned,
efc_bonus_low, efc_bonus_high, tourney_banned, tourney_max_evo_banned, id_artist, id_artist_re,
release_date, is_ile_available, is_re_available, is_first_evo_released, evos, bonus, nightBonus,
activeBoosterItems, collectionData, marketData`

Trimmed example (A Award Cr, one evo shown):

```json
{
 "id": 317, "name": "A Award Cr", "clan_id": 30, "clan_name": "Sakrohm",
 "level_min": 1, "level_max": 3, "rarity": "cr", "rarity_value": 4, "kind": "collector",
 "is_clan_leader": false, "efc_banned": false, "efc_max_evo_banned": false, "efc_temp_banned": false,
 "efc_bonus_low": false, "efc_bonus_high": false, "tourney_banned": false, "tourney_max_evo_banned": false,
 "evos": {
  "3": { "riftPower": 90, "pictureURL": "https://s.acdn.ur-img.com/characters/....png", "HDPictureURL": "...",
         "power": 3, "damage": 4,
         "ability": { "id": 158, "typeID": 20, "unlockLevel": 3,
                      "description": "Courage: Power +3",
                      "longDescription": "When played first in the round, A Award Cr's Power is increased by 3." },
         "nightAbility": [] }
 },
 "bonus": { "id": 29, "typeID": 4, "description": "-8 Opp Attack, Min 3", "longDescription": "..." },
 "nightBonus": [],            // or {"id":51,"typeID":210,"description":"Night: -1 Opp Pow. And Damage, Min 1",...}
 "activeBoosterItems": [],    // e.g. ["bpoints"]
 "collectionData": {
   "id": 317, "time_last_update": 0, "time_last_acquisition": 0,
   "lvl_1": 0, "lvl_2": 0, "lvl_3": 0, "lvl_4": 0, "lvl_5": 0,          // plain copies owned, per level
   "lvl_1_p": 0, ... "lvl_5_p": 0,  "time_last_acquisition_p": 0,        // Prismatic
   "lvl_1_s": ..., "lvl_1_m1": ..., "lvl_3_m1": 1, ..., "lvl_1_m2": ..., "lvl_1_m3": ...,
   "lvl_1_rp": ..., "lvl_1_i": ...                                        // two more states, never non-zero here
 },
 "marketData": {
   "id": 317, "last_sale_time": 1790189527, "min_price": 17000000,
   "lvl_1": 3, "price_lvl_1": 17000000, ..., "lvl_3": 1, "price_lvl_3": 17400798, ...  // offers on sale + cheapest, per level
   "..._p", "..._s", "..._m1", "..._m2", "..._m3", "..._rp", "..._i"                     // same per state
 }
}
```

What a card record carries, against the brief's checklist:

- character id, name, clan, rarity, kind, level range, per-level power/damage/ability, bonus,
  night variants, artwork URLs: **yes** (a strict superset of `characters.get`, plus structured
  ability ids/type ids, which `data/site_characters.jsonl` lacks).
- owned copies per (level, state): **yes**, `collectionData.lvl_<L>[_<state>]`. States seen:
  `""` (3219 copies), `m1` (100), `p` (43), `s` (1); keys also exist for `m2`, `m3`, `rp`, `i`.
  Each copy's level is its whole identity: there is no per-copy id and no per-copy XP.
- XP: **no per-card XP**. Evolving spends the player's XP reserve (`evolve` response shows
  `XPReserve.currentXP` 44000 -> 43500 for a level 1 -> 2 evo; `general.config.evoCostTable` =
  `{"2":500,"3":1500,"4":3000,"5":5000}`).
- in-deck flags: **not present**; must be derived from the deck list (section 2).
- locked / for-sale flags for the owner's own copies: **not present**. `marketData` is the public
  market, not the owner's listings.
- ban flags for formats: **yes** (see section 3; they match the format definitions exactly).

The market page's `/ajax/market/ action=marketdata&section=public&marketPlayerID=0&page=N&nbPerPage=500`
returns the **same 36-key schema** (catalog + the owner's `collectionData` + `marketData`), so either
call can seed a deck builder.

### 1b. Game client: private API `collections.get`

```
requests=[{"call":"collections.get","params":{"page":0,"nbPerPage":1000}}]   (pages 0,1,2)
-> {"collections.get":{"data":{"collection":[ ... 1000 rows ... ], "hasNextPage": true}}}
```

Rows are ownership only, camelCase, one per character (all 2497-2498 characters, owned or not):

```json
{"id": 1334, "timeLastUpdate": 0, "timeLastAcquisition": 0,
 "lvl1": 2, "lvl2": 1, "lvl3": 0, "lvl4": 0, "lvl5": 0,
 "lvl1P": 0, ..., "lvl1S": 0, ..., "lvl1M1": 0, ..., "lvl1M2": 0, ..., "lvl1M3": 0, ..., "lvl1Rp": 0, ..., "lvl1I": 0, ...,
 "timeLastAcquisitionP": 0, ...,
 "minPrice": 16200700, "minPriceP": 19999995, "minPriceS": 110798110, "minPriceM1": 18000000,
 "minPriceM2": 52499997, "minPriceM3": 144999999, "minPriceRp": 0, "minPriceI": 0}
```

Seen 73 times (paged, with `hasNextPage`). This is the lighter call for "what do I own at which level";
it joins with `characters.get` (which the userscript already dumps) on `id`.

### 1c. Single character: `/ajax/characters/ action=getcharacter&id=845&level=2&state=`

Returns `{"character":{id, name, url, clan_id, clan_name, level, xp_for_level, level_min, level_max, power,
damage, rarity, ability_id, ability, ability_unlock_level, bonus, has_night_bonus, bank_price, distrib, kind,
state, offer_at_level, release_date, efc_banned, efc_max_evo_banned, efc_temp_banned, efc_bonus_low,
efc_bonus_high, tourney_banned, tourney_max_evo_banned, penalty, collector_date, is_ultra, is_meteora, ...,
pictureURL, ...}}` (flat, one level, HTML in `ability` for locked abilities). 11 calls, old log only.

## 2. Decks

### 2a. List: private API `collections.decks` (game client only)

```
requests=[{"call":"collections.decks","params":{"deckFormatID":54363}}]
   params seen: {} (89), {"deckFormatID":0} (104), 1 (110), 54363 (151), 55009 (92), 57215 (89)
-> {"collections.decks":{"data":{"decks":[
     {"id": 37646105, "playerId": 19309601, "name": "T1 Pirhanas",
      "characters": [{"id":528,"level":3,"state":""}, {"id":516,"level":3,"state":"m1"}, ...8 entries],
      "isCurrent": true},
     ... ]}}}
```

The account had 19 decks (`general.initPlayer`/`refreshPlayer` report `maxDecks: 21`).
**Correction, 2026-09-26:** the `deckFormatID` parameter does filter. That day's captures return
all 19 decks for `{}`, `0` and Free Fight `57215`, but only the decks legal in the format for
the others: Tourney `54363` 7, EFC `1` 8, Survivor `55009` 4. That answer is the server's own
legality verdict, and `scripts/DeckCapture.ts` records it (`legalByFormat` in
`data/my_decks.json`) as the oracle for `src/decks/DeckFormat.ts`, which agrees on all 76
deck-format pairs. The earlier reading of "no filtering" came from logs where the same 19
came back every time. A deck record is
only `{id, playerId, name, characters[{id, level, state}], isCurrent}`: no format id, no favourite,
no creation time.

The web page's deck list (`/collection/decks/list.php`, visited 4 times) and deck view
(`/collection/decks/?id=17990965`) fired **no** list XHR: they are server-rendered HTML, which the
logger does not capture.

### 2b. Load one: `/ajax/collection/ action=loaddeck&id=37646105`

```json
{"deck":{"id":37646105,"playerID":19309601,"name":"T1 Pirhanas",
  "Characters":[{"id":528,"level":3,"state":""},{"id":516,"level":3,"state":"m1"}, ...],
  "isCurrent":true}}
```

(Note the casing differs from the API: `playerID`, `Characters`.) Collection Pro always loads the
current deck on open; the id it asks for must come from the page HTML.

### 2c. Create / save / rename: `/ajax/collection/ action=savedeck` (2 captures, old log)

```
action=savedeck&id=0&name=T1+Pirhanas&set_current=true
  &characters[0][id]=566&characters[0][level]=4&characters[0][state]=
  &characters[1][id]=727&characters[1][level]=3&characters[1][state]=
  &characters[2][id]=516&characters[2][level]=3&characters[2][state]=m1
  ... (8 entries)
-> {"deck":{"id":37646105,"playerID":19309601,"name":"T1 Pirhanas","Characters":[...],"isCurrent":true}}
```

- `id=0` creates a deck (the new id came back as 37646105; older decks are ~17.6M-18.3M).
- `id=<existing>` overwrites it (seen with `id=17990965&name=T1+Riots&set_current=false`), so rename
  and every add/remove are a full-deck `savedeck`; there is no per-card add/remove call.
- `set_current=true|false` makes it the active deck in the same request.
- Server reorders the cards in its reply; order is not meaningful.
- Name limit: `general.config.maxNameLengthDeck = 32`.
- No server-side validation response for an illegal deck was captured.

### 2d. Set active deck

- Web: `/ajax/collection/ action=setcurrentdeck&id=17990965` -> `{"success":true}` (1 capture).
- Game: `collections.setCurrentDeckID {"id":18181425}` -> `{"data":null}` (33 captures).

There is **one global current deck**, not one per room: the game client calls
`collections.decks` then `setCurrentDeckID` after the player picks a deck for the room they are
entering (e.g. `collections.decks {deckFormatID:54363}` at t=1789232289516 then
`setCurrentDeckID {id:17995223}` 11 s later). The room itself stores no deck.

### 2e. Delete

Not in either log, but in Collection Pro's own code (read live 2026-09-26): the deck list's
delete button (`.js-deck-delete`, in the "Load a deck" modal) asks "Are you sure you want to
delete this deck?" in the site's own modal, then posts `action=deletedeck&id=<deck id>` to
`/ajax/collection/` and expects `{"success":true}` or `{"error"|"fatal_error": "..."}`.

### 2f. Presets (community decks)

A "preset" concept exists: `/presets/?id=2908448` pages and preset objects embedded in
`/ajax/spot/ action=getUserFeed` posts:

```json
"preset": {"id": 2908448, "player": {"id": 28469642, "name": "Mr- Cold", ...},
  "url": "/presets/?id=2908448", "name": "Selka Riv in EFC!", "description": "...",
  "deckFormat": {"id": 1, "name": "Type EFC", "isOfficial": true},
  "isStillCompatible": true, "date_created": "2026-09-07 17:59:16",
  "starSum": 25, "powerSum": 58, "damageSum": 34, "totalCharacters": 8, "score": 120,
  "clans": [{"id":51,"name":"Hive"},{"id":56,"name":"Oculus"},{"id":60,"name":"Tolvack"}],
  "cards": [{"id": 2689, "name": "Atel\u00f8ps-X", "level": 3, "power": 8, "damage": 4,
             "ability": "Bet > 3 Pillz: -2 Opp. Life Min 0", ...}, ...]}
```

`general.config` has `minLevelPublishPreset: 6`, `minCharactersPublishPreset: 8`,
`maxCharactersPublishPreset: 12`, `maxNameLengthPublishPreset: 64`. No preset list/search or
"publish preset" call was captured. No "favourite deck" field exists anywhere in the logs.

### 2g. Quick battle deck

`battles.quickBattleDeck {}` -> `{"data":{"charactersList":[]}}` (2 captures, empty). Purpose unclear.

## 3. Deck formats and room rules

### 3a. Format definitions: `/ajax/collection/ action=deckformatsdata` (no params)

This answers the backlog's "format rules themselves" question. Response on 2026-09-23 (4 formats):

| id | name | criteria (`name` = `value`) |
| --- | --- | --- |
| 57215 | Free Fight | `min_characters`=8, `no_doubles`=true |
| 54363 | Tourney | `min_characters`=8, `max_stars`=32, `forbidden_character_list`=[218 ids], `forbidden_maxed_character_list`=[133 ids], `no_doubles`=true |
| 1 | EFC | `min_characters`=8, `max_stars`=25, `forbidden_character_list`=[4 ids: 2701 Dorga, 2671 Krym, 2706 Trasher-X, 2166 Madrat], `forbidden_maxed_character_list`=[198 ids], `max_level1_characters`=0, `max_level5_characters`=1, `exclude_elo_forbidden`=true, `no_doubles`=true |
| 55009 | Survivor | `min_characters`=10, `no_doubles`=true |

Shape: `[{"id":54363,"name":"Tourney","isOfficial":true,"criteria":[{"name":"max_stars","description":"The sum of the card levels in your Deck must not exceed 32.","value":32}, ...]}]`.
Each criterion carries a human description (`forbidden_*` ones list every card name) and a
machine `value` (int, bool, or list of character ids). "Stars" = card level. "Maxed" = at
`level_max`.

The lists move: EFC `forbidden_character_list` was 5 ids on 2026-09-09 (Dorga, Leander, Drava,
Krym, Velvet Lamento), 4 on 09-14 (Leander and Trasher-X in), 4 different ones on 09-23;
Tourney maxed-ban went 131 -> 132 -> 133. Fetch it fresh, never hard-code.

Cross-checks against the collection flags (2026-09-23 snapshot, exact set equality):

- Tourney `forbidden_character_list` == characters with `tourney_banned` (218).
- Tourney `forbidden_maxed_character_list` == `tourney_max_evo_banned` (133).
- EFC `forbidden_maxed_character_list` == `efc_max_evo_banned` (198).
- EFC `forbidden_character_list` == `efc_temp_banned` (4) - the weekly EFC vote bans.
- `efc_banned` (267 characters: 158 normal, 71 Cr, 23 Oculus, 15 Leaders) is disjoint from the
  temp list and is the likely meaning of `exclude_elo_forbidden` ("cards banned by staff in ELO
  mode"). Supporting evidence: across the 67 captured EFC games (536 hand cards) **no** card has
  `efc_banned`, none is maxed-banned at max level and none is level 1; across 268 Tourney games
  (2144 hand cards) none violates the Tourney lists. That is consistent, not proof.
- `efc_bonus_low` (102) / `efc_bonus_high` (0): meaning unknown (maybe EFC scoring modifiers).

Leaders: 21 characters with `kind:"leader"` (clan "Leader"); 36 more have `is_clan_leader:true`
(ordinary cards). No criterion limits Leaders per deck in any format; 15 of the 21 Leaders are
`efc_banned`. Clan restrictions: none in any criterion. Deck size maxima: none seen
(`min_characters` only), though the owner's decks top out at 10.

### 3b. Rooms: private API `rooms.list {}` (123 calls, identical content every time)

```json
{"rooms.list":{"data":{"rooms":[
  {"id":13044,"name":"Free Fight","description":"Free Fight / PVP / Free deck, 8+ cards without duplicates / ...",
   "pictureUrl":"...","minLevel":7,"maxLevel":0,"idDeckFormat":57215,"idBattleRule":1,"currentRoom":false,"deckFormat":[]},
  ...], "theRiftDescription":"..."}}}
```

| room id | name | idDeckFormat | idBattleRule | minLevel |
| --- | --- | --- | --- | --- |
| 154844 | Dojo | 0 | 6 | 0 |
| 13044 | Free Fight | 57215 | 1 | 7 |
| 16193 | Tourney | 54363 | 10 | 7 |
| 5 | EFC | 1 | 3 | 7 |
| 1202394 | Survivor | 55009 | 4 | 7 |
| 6 | Training | 0 | 2 | 0 |

`deckFormat` was always `[]` (the rules are only in `deckformatsdata`). `idDeckFormat 0` = no format
(Training "Free deck, 8+ cards", Dojo "No deck required"). Room descriptions carry rules text that
matters for deck choice, e.g. Tourney: "Points are earned by playing quickly, winning rounds, and
having fewer stars than the opponent" (so a low-star Tourney deck scores more). `rooms.join {"id":16193}`
returns the same room object; `rooms.playerData` returns per-battle-rule standing
(`{"battleRules":[{"id":3,"playerData":{"rank":1,"league":1}},{"id":4,"playerData":{"score":0}},{"id":10,"playerData":{"rank":0}},...]}`).
Captured battles carry `room.idDeckFormat` (Tourney 268, EFC 67, Training/Dojo 16, Free Fight 3,
Survivor 3, 26 older without room info).

### 3c. Other format-relevant sources

- `rooms.EFCListVotes {}` (1 capture): `isVoteOpen`, `charactersDataToVote[{idCharacter, nbVoteFor,
  nbVoteAgainst, total}]`, `nbVoteForAllCharacters`, `playerVotedCharactersData`,
  `maxedCharactersIdsBanned[...]` - the EFC weekly ban vote, i.e. next week's temp bans.
- `general.config.tourney.planning[{startUnixtime,endUnixtime}]` - the 30-minute Tourney slots.
- `therift.data`: 7-card Rift mode with 8 factions and 8 leader modifiers
  (`leaders[{id, idAffects, idAffectsPosition, value, valuePrismatic}]`) - a separate game mode,
  not a deck format.

## 4. Market and prices (brief)

| Call | Request | Response |
| --- | --- | --- |
| `marketdata` | `action=marketdata&section=public&marketPlayerID=0&page=N&nbPerPage=500` | catalog rows as in 1a; `marketData` = offers on sale and cheapest price per (level, state), `last_sale_time` |
| `trendssalesdata` | `action=trendssalesdata&section=public&marketPlayerID=0&charactersIDs[]=1131&...(10 ids)&nbSales=25&salesPage=0&onlyState=all&onlyMissingEvos=0` | `[{id, evos{L:{power,damage}}, trendsData{day,week,month,year:{before,after,evolution%}}, salesData[{id(saleID), price, time, character{id,level,state}, player{id,name}}]}]` - current offers, cheapest first |
| `pricehistory` | `action=pricehistory&id=1476&state=&period=week` | `{"period":"week","kind":"sales","points":[{time, level, price}, ...]}` - completed sales |
| `purchase` | `action=purchase&saleID=855495505` | `{"saleData":{id, price, time, newMinPrice, character{id,level,state}, player{id,name}}, "player":{level, clintz, credits, XPReserve,...}}` |
| `sell` | `action=sell&id=279&level=1&state=&quantity=2&price=101747&type=kate|public&recipient=` | `{nbSalesSuccessfull, nbSalesUnsuccessfull, errorMessages[], player{...}, collectionCharacter{...collectionData...}}`; `type=kate` sells to the bank at the `characterbankdata` price |
| `characterbankdata` | `/ajax/collection/ action=characterbankdata&id=279` | `{"newTimer":ms, "newTimerSeconds":249, "bankData":{"":101747,"p":700000,"s":700000,"m1":700000,"m2":50,"m3":50}}` (bank buy-back price per state, refreshes every ~5 min) |
| `evolve` | `/ajax/collection/ action=evolve&id=445&level=1&state=m1&quantity=1` | `{player{...XPReserve}, collectionCharacter{...}}` |

`collections.get` (1b) also carries `minPrice*` per state, enough for a "cost to complete this deck"
estimate without touching the market pages. `general.config`: `maxSalePrice`, `nbDaysExpirationSales: 7`,
`clintzFor100CharacterXp: 3188`.

## 5. Every private-API method seen (new log / old log)

Counts are requests (`new` = 2,865 form-encoded calls; `old` = 32,124 form + 249 binary/empty,
attributed by response key).

| Area | Method | new | old | Notes |
| --- | --- | ---: | ---: | --- |
| auth | `auth.exchangeToken` | 6 | 29 | params `{code, platform, verifier}` -> `{accessToken, accessTokenExpiresIn:1800, refreshToken}` |
| auth | `auth.refresh` | 4 | 37 | params `{refreshToken}` |
| battles | `battles.status` | 753 | 10267 | capture pipeline's main input |
| battles | `battles.play` | 99 | 1228 | |
| battles | `battles.ongoingBattleID` | 63 | 1018 | |
| battles | `battles.quickBattle` | 21 | 387 | matchmaking poll, `{}` |
| battles | `battles.result` | 26 | 354 | |
| battles | `battles.create` | 17 | 169 | `{opponentID, isQuickBattle}` -> `{battle:{id}}` |
| battles | `battles.stopQuickBattle` | 0 | 12 | |
| battles | `battles.quit` | 0 | 3 | |
| battles | `battles.quickBattleDeck` | 0 | 2 | `{}` -> `{charactersList:[]}` |
| ccpass | `ccpass.get` / `.data` / `.progressed` / `.setSeen` / `.dialogues` | 7/7/0/0/0 | 35/29/3/3/3 | battle pass |
| characterdialogues | `characterdialogues.getForBattle` | 22 | 356 | |
| characters | `characters.get` | 12 | 113 | `{page, timestampLastUpdate}`; used by `__ur.dumpCharacters()` |
| clans | `clans.get` | 7 | 29 | always `{"clans":[]}` (not the card clans) |
| collections | `collections.certificates` | 74 | 908 | `{certificates:[], ileCertificates:[], reCertificates:[]}` |
| collections | `collections.decks` | 46 | 603 | deck list, section 2a |
| collections | `collections.get` | 14 | 59 | ownership, section 1b |
| collections | `collections.setCurrentDeckID` | 1 | 32 | section 2d |
| general | `general.refreshPlayer` | 68 | 880 | player clintz/credits/level/`maxDecks` |
| general | `general.config` | 7 | 29 | global limits, evo costs, Tourney planning |
| general | `general.initPlayer` | 7 | 29 | player profile incl. `maxDecks`, `efcLeague` (also contains the account email: do not log/publish) |
| general | `general.getSettings` / `general.setLocale` / `general.loginToSocketServer` | 7/6/6 | 29/28/30 | |
| missions | `missions.progressed` / `.progressedInBattle` / `.reachLevel` | 59/26/0 | 807/354/1 | |
| notifications | `notifications.get` | 66 | 440 | |
| rankings | `rankings.getShort` / `rankings.get` | 20/0 | 342/1 | |
| rooms | `rooms.list` / `.playerData` / `.join` / `.onlinePlayers` / `.randomAI` / `.leave` / `.EFCListVotes` | 14/13/8/7/7/2/0 | 113/112/104/101/83/45/1 | section 3b/3c |
| slotmachine | `slotmachine.uniqueReward` / `.spin` / `.get` | 23/0/0 | 1148/3/2 | |
| stickers | `stickers.data` / `stickers.get` | 11/7 | 29/29 | |
| teambattles | `teambattles.clans` | 14 | 74 | |
| teams | `teams.get` | 1303 | 11850 | polled constantly, always `{"data":[]}` |
| therift | `therift.data` | 5 | 29 | |
| welcome | `welcome.send` | 0 | 1 | |

Other endpoints: `GET /api/clientdata/` -> client versions (`{"ur_webgl":"2.0.3-final2",...}`).
`/ajax/` actions seen: `news getbreakingnews` (4002), `player setTimeZone` (189),
`notifications updatebadge` (169) / `setread` (140), `collection characterbankdata` (141) /
`collectiondata` (98) / `loaddeck` (27) / `deckformatsdata` (26) / `evolve` (7) / `savedeck` (2) /
`setcurrentdeck` (1), `player/account webgllogincode` (34, returns `{"code":"<redacted>"}`),
`market marketdata` (30) / `sell` (28) / `trendssalesdata` (21) / `purchase` (8) / `pricehistory` (2),
`characters getcharacter` (11), `spot getFeed` (3) / `getUserFeed` (2), `ccpass setseen` (1).

## 6. How the site calls its private API, and what the userscript can reuse

- Auth chain (inferred from order and params): the web page (cookie session) calls
  `/ajax/player/account/ action=webgllogincode` -> `{code}`; the Unity client calls
  `auth.exchangeToken {code, platform, verifier}` -> access token valid 1800 s + refresh token;
  later `auth.refresh {refreshToken}`. The access token is not in the request body or URL, so it
  travels in a request header. The logger never records request headers (it only keeps the last
  API call's `Headers` in memory as `lastApiInit`, ur-logger.user.js ~line 205), which is why no
  token appears in request records - and why the header name is unknown from the logs.
- `__ur.apiCall(call, params)` (ur-logger.user.js ~line 222) replays `lastApiInit` with a new
  `requests=` body. That only works in a tab where the game client has already made one API call,
  i.e. `/game/play/webgl/`. **On `/collection/pro/` it will not work by itself**: that page talks
  only to `/ajax/*` with cookies. For a Collection Pro integration the `/ajax/collection/` actions
  (`collectiondata`, `deckformatsdata`, `loaddeck`, `savedeck`, `setcurrentdeck`) are the natural
  surface: same origin, cookie auth, plain form posts, no token handling needed.
- Page attribution in this log is approximate: `page` records are per tab but other records carry
  no tab id, so API calls "during" a Collection Pro visit may come from a concurrently open game tab.

## 7. Not found in the logs (needs a live look at the page)

Items 1, 2, 6 and 9 were answered by the live look on 2026-09-26; see section 8.

1. Deck deletion (no call captured) and the deck-list page's own data (`/collection/decks/list.php`
   and `/collection/decks/?id=` are server-rendered HTML; no XHR).
2. How Collection Pro learns the page count for `collectiondata` and the current deck id it passes
   to `loaddeck` (both presumably in the HTML/inline JS; the logger does not keep page HTML).
3. Any server-side deck validation error shape (no rejected `savedeck` captured); whether the server
   rejects an illegal deck on `savedeck`, on `setCurrentDeckID`, or only when joining/matchmaking.
4. Whether `savedeck` also accepts a format id or whether decks are format-less (they appear
   format-less everywhere).
5. The owner's own market listings (copies on sale/locked), and whether copies in a deck can be sold.
6. The meaning of `efc_bonus_low` / `efc_bonus_high`, the `rp` and `i` states, and whether
   `efc_banned` is exactly the "ELO forbidden" list (strongly suggested, not confirmed).
7. Preset listing/search/publish calls and the `/presets/` page's data.
8. The request header that carries the private-API access token (only observable in devtools or by
   extending the logger to record header names, with values redacted).
9. Collection Pro's own client-side JS (filters, drag-and-drop) - relevant if the userscript is to
   patch or replace the page UI rather than call the endpoints itself.

## 8. Live look at Collection Pro (2026-09-26, the owner's Edge, read-only)

Taken through Claude in Chrome with nothing saved, sold or evolved.

**Deck editor state.** The loaded deck's id, name and current flag sit on the Save button:
`.js-deck-save[data-id][data-name][data-iscurrent]`. Each card of the deck being edited, saved
or not, is an `li` in `.js-deck-cards-list` whose `a.js-load-character` carries
`data-character-id`, `data-character-level` and `data-character-state`. The format dropdown
is `select.js-deck-format-filter` (`0` none, `57215`, `54363`, `1`, `55009`); choosing one only
runs the client-side validator, it sends nothing. Collection rows are
`tr#accordion-pro-header-<card id>`. The page sets `window.isNight`.

**The site's own validator.** `DeckFormat.parseDeck` in `collection-pro-bundle.min.js` checks a
deck against a format's criteria in the browser; `src/decks/DeckFormat.ts` is a port. Criteria
it knows: `min|max_characters`, `min|max_stars` (stars = sum of levels), `max_level1..5_characters`,
`max_commons|uncommons|rares|ld|cr|mt_characters`, `no_collectors`, `min|max_leaders` (clan 36),
`min|max_clans` (Leaders excluded), `min|max_character_level`, `min|max_evolving_characters`,
`min|max_maxxed_characters`, `min|max_release_date`, `no_doubles`, `exclude_elo_forbidden` (the
card's `efc_banned`), `authorized|contained|forbidden_clan_list`,
`authorized|contained|forbidden_character_list`, `forbidden_maxed_character_list`,
`authorized|contained|forbidden_ability_type_list` (the level's ability `typeID`, 0 ignored) and
`force_balanced_clans`. It ignores any other name silently.

**Other actions in the bundle** (`/ajax/collection/` unless noted): `collectionoptions` (on
`/ajax/player/`), `collectiondata`, `latestcollectiondata`, `certificatesdata`,
`characterbankdata`, `deckformatsdata`, `loaddeck`, `savedeck`, `deletedeck`, `setcurrentdeck`,
and HTML fragments from `/ajaxcontent/decks/my-decks.php`, `autodeck-clans.php`,
`autodeck-generate.php` and `autodeck-rooms.php`. No request carries a CSRF token; jQuery's
`$.post` relies on the session cookie.

**Collection changes (documented, never automated).** The card "Manage" window holds two forms:
- `/ajax/collection` `action=evolve` (or `devolve`, allowed only for Immortal Legacy Edition
  copies) with `id`, `level`, `state`, `quantity`. It spends XP reserve first, then Clintz, and
  answers with the updated `collectionCharacter` and `player`.
- `/ajax/market` `action=sell` with `id`, `level`, `state`, `quantity`, `serial` (a chosen serial
  number), `price`, `type` (public sale, sell to Kate - the bank - or a private sale) and
  `recipient` for a private sale.
Both change the account irreversibly. The deck builder must never call them; its safety rules
are in `docs/deck-builder-design.md`.

**Editions** (the `only_state` filter): `""` Classic, `p` Prismatic, `s` Savage, `ga` Golden
Aura, `gs` Golden Savage, `m1` Sapphire Deep Blue, `a1` Andromeda, `k1` Knight Blaze, `v1` Void,
`c1` Cold, `g1` Glam, `ar1` Arcade, `m2` Frost Platinum, `m3` Wonder Spectrum, `rp` Rampage,
`i` Immortal Legacy. So `m1`/`m2`/`m3` are three named editions, not tiers of one.

**Local network access.** A page on the site can reach the log server on `localhost:8787`
(the owner allowed it). The first request to another local port (`8788`) left the tab waiting
on Edge's local-network prompt with nobody there to answer, freezing its scripts; the deck
panel therefore goes through the log server's `/decks/` proxy.

**Userscript on Edge.** Tampermonkey needs the extension's "Allow user scripts" toggle; neither
`chrome-extension://` nor `edge://` pages can be driven by browser automation, so installing or
updating the script (open http://localhost:8787/ur-logger.user.js) and flipping that toggle are
the owner's clicks. `__ur.dumpCharacters()` must run in a `/game/play/webgl/` tab, whose private
API calls it borrows.
