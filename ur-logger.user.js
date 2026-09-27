// ==UserScript==
// @name         UR logger
// @namespace    urban-recreation
// @version      0.9.2
// @description  Mirror Urban Rivals network traffic to a local log server (see log_server.ts)
// @match        https://www.urban-rivals.com/*
// @run-at       document-start
// @grant        none
// @updateURL    http://localhost:8787/ur-logger.user.js
// @downloadURL  http://localhost:8787/ur-logger.user.js
// ==/UserScript==
//
// The script is deliberately a dumb pipe: it captures every fetch / XHR / WebSocket
// message and POSTs it to http://localhost:8787/log. All interpretation (which API
// method was called, which battle it belongs to, what to keep) lives in log_server.ts,
// so it can be changed without re-installing the userscript.
//
// Record shape: { t: epoch ms, kind: 'fetch'|'xhr'|'ws_open'|'ws_in'|'ws_out'|'page', payload }
//   fetch/xhr payload: { m, u, body?, status, resp }
//   Request bodies that are not text are decoded as UTF-8 when possible, otherwise sent
//   as 'b64:<base64>' so nothing is lost. Response bodies that are not text are not sent
//   at all (see respText).
(() => {
  // Keep equal to @version above; log_server.ts compares it with the repository copy and
  // says when this one is out of date.
  const VERSION = '0.9.2';
  const SERVER_ROOT = 'http://localhost:8787';
  const SERVER = SERVER_ROOT + '/log';
  const CONTROL = SERVER_ROOT + '/control';
  const nativeFetch = window.fetch.bind(window); // saved BEFORE we patch anything
  // WebSocket frames arrive in order, but firing one independent fetch per frame allowed
  // duplicate hover enter/leave messages to reach localhost out of order. Keep that small
  // stream serial so the capture server sees the same order as the game client.
  let wsInLog = Promise.resolve();
  const logWsIn = (payload) => {
    const ready = Promise.resolve(payload);
    wsInLog = wsInLog.then(() => ready.then((value) => log('ws_in', value)));
  };
  let apiCall;
  let autoQueue = false;
  let lastBattleId = 0;
  let lastQueuedBattleId = 0;

  // keepalive lets records survive page navigation, but browsers cap keepalive bodies at
  // 64 KiB and silently reject larger ones, so only use it for small records.
  const log = (kind, payload) => {
    const body = JSON.stringify({ t: Date.now(), kind, payload });
    return nativeFetch(SERVER, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body,
      keepalive: body.length < 60000,
    }).then((r) => { if (!r.ok) console.warn('UR logger: server returned', r.status, kind); })
      .catch((e) => { if (kind === 'characters') console.warn('UR logger: failed to send', kind, e); });
  };

  // Control stays in the local log server so the advisor and browser share one state. It
  // is OFF after a server restart and otherwise remains enabled across consecutive games.
  const syncControl = async () => {
    try {
      const res = await nativeFetch(CONTROL, { cache: 'no-store' });
      if (res.ok) autoQueue = !!(await res.json()).autoQueue;
      else autoQueue = false;
    } catch {
      autoQueue = false; // loss of the local controller always fails safe
    }
  };
  syncControl();
  setInterval(syncControl, 500);

  // ---- body decoding -----------------------------------------------------------------
  const b64 = (bytes) => {
    let s = '';
    for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode.apply(null, bytes.subarray(i, i + 0x8000));
    return 'b64:' + btoa(s);
  };
  const decodeBytes = (bytes) => {
    const txt = new TextDecoder('utf-8', { fatal: false }).decode(bytes);
    // Replacement char means it was not UTF-8 text (compressed / msgpack / ...). Keep the raw bytes.
    return txt.includes('�') ? b64(bytes) : txt;
  };
  // ---- response bodies worth mirroring ------------------------------------------------
  // The game streams WebGL asset bundles (UnityFS, several MB each), images and wasm through
  // fetch. Reading one with res.text() decodes it as lossy UTF-8: about a third of the result
  // is U+FFFD, so the bytes are already destroyed and the log entry can never be used for
  // anything. Unfiltered they were 65% of the first real capture log — 3.5 GB of 5.4 GB, from
  // 289 of 26227 lines. Keep the request line, drop the body. log_server.ts applies the same
  // rule to anything that still gets through, so old versions of this script stay safe.
  const MAX_RESP = 4 * 1024 * 1024;
  const TEXTUAL = /^(?:$|text\/|application\/(?:json|javascript|xml|x-www-form-urlencoded))/i;
  const capped = (txt, what) =>
    txt.includes('�') ? `[skipped: ${txt.length} chars of binary ${what}]`
      : txt.length > MAX_RESP ? `[skipped: ${txt.length} chars of ${what}]`
      : txt;
  // Cross-origin responses can hide their headers; then the checks above catch it after the
  // read instead, which is why the U+FFFD test and not just the content type is needed.
  const respText = async (res) => {
    const type = (res.headers.get('content-type') || '').split(';')[0].trim();
    const len = Number(res.headers.get('content-length') || 0);
    if (!TEXTUAL.test(type)) return `[skipped: ${type}]`;
    if (len > MAX_RESP) return `[skipped: ${len} bytes of ${type || 'unknown type'}]`;
    return capped(await res.clone().text(), type || 'unknown type');
  };

  const bodyText = async (body, request) => {
    try {
      if (body == null) {
        if (request instanceof Request && request.method !== 'GET' && request.method !== 'HEAD') {
          return decodeBytes(new Uint8Array(await request.clone().arrayBuffer()));
        }
        return undefined;
      }
      if (typeof body === 'string') return body;
      if (body instanceof URLSearchParams) return body.toString();
      if (body instanceof FormData) return JSON.stringify(Object.fromEntries(body));
      if (body instanceof Blob) return decodeBytes(new Uint8Array(await body.arrayBuffer()));
      if (body instanceof ArrayBuffer) return decodeBytes(new Uint8Array(body));
      if (ArrayBuffer.isView(body)) return decodeBytes(new Uint8Array(body.buffer, body.byteOffset, body.byteLength));
      if (body instanceof ReadableStream) return '[stream]';
      if (typeof body === 'object' && 'byteLength' in body) return decodeBytes(new Uint8Array(body)); // cross-realm buffers
      return `[${Object.prototype.toString.call(body)}]`;
    } catch (e) {
      return `[body error: ${e && e.message}]`;
    }
  };

  // The browser owns the authenticated API session. When a finished battle is observed,
  // queue exactly once through the same request helper the manual data tools use.
  const inspectApiResponse = (text) => {
    let json;
    try { json = JSON.parse(text); } catch { return; }
    const status = json['battles.status'] && json['battles.status'].data;
    if (status && status.battle && status.battle.id) lastBattleId = status.battle.id;
    const result = json['battles.result'] && json['battles.result'].data;
    if (!result || !result.battle) return;
    const battleId = lastBattleId || Date.now();
    // Refresh at the decision point rather than trusting a value that can be 500ms old.
    syncControl().then(() => {
      if (!autoQueue || lastQueuedBattleId === battleId) return;
      lastQueuedBattleId = battleId;
      setTimeout(async () => {
        await syncControl();
        if (!autoQueue) return;
        try {
          await apiCall('battles.quickBattle', {});
          console.log('UR logger: auto-queue requested');
        } catch (e) {
          console.warn('UR logger: auto-queue failed', e);
        }
      }, 750);
    });
  };

  // ---- WebSocket: both directions -----------------------------------------------------
  const NativeWS = window.WebSocket;
  window.WebSocket = function (url, protocols) {
    const ws = protocols ? new NativeWS(url, protocols) : new NativeWS(url);
    log('ws_open', String(url));
    ws.addEventListener('message', (e) => {
      if (typeof e.data === 'string') logWsIn(e.data);
      else if (e.data instanceof ArrayBuffer) logWsIn(decodeBytes(new Uint8Array(e.data)));
      else if (e.data instanceof Blob) logWsIn(e.data.arrayBuffer().then((ab) => decodeBytes(new Uint8Array(ab))));
      else logWsIn('[binary]');
    });
    const send = ws.send.bind(ws);
    ws.send = (data) => {
      bodyText(data).then((txt) => log('ws_out', txt));
      return send(data);
    };
    return ws;
  };
  window.WebSocket.prototype = NativeWS.prototype;
  for (const k of ['CONNECTING', 'OPEN', 'CLOSING', 'CLOSED']) window.WebSocket[k] = NativeWS[k];

  // ---- XHR -------------------------------------------------------------------------------
  const open = XMLHttpRequest.prototype.open;
  const send = XMLHttpRequest.prototype.send;
  XMLHttpRequest.prototype.open = function (m, u) {
    this._ur = { m, u: String(u) };
    return open.apply(this, arguments);
  };
  XMLHttpRequest.prototype.send = function (body) {
    const bodyP = bodyText(body);
    this.addEventListener('load', () => {
      let resp;
      try {
        resp = this.responseType === '' || this.responseType === 'text' ? capped(this.responseText, 'text')
          : this.responseType === 'json' ? JSON.stringify(this.response)
          : this.responseType === 'arraybuffer' ? capped(decodeBytes(new Uint8Array(this.response)), 'arraybuffer')
          : `[${this.responseType}]`;
      } catch { resp = `[${this.responseType}]`; }
      bodyP.then((b) => log('xhr', { ...this._ur, body: b, status: this.status, resp }));
    });
    return send.apply(this, arguments);
  };

  // ---- fetch -----------------------------------------------------------------------------
  // Remember how the site itself calls its private API so __ur.dumpCharacters() can reuse
  // the exact same headers / credentials (whatever carries the auth) with a different body.
  const API = 'https://www.urban-rivals.com/api/private/v2/';
  let lastApiInit = null;
  window.fetch = async (...args) => {
    const [input, init] = args;
    const url = String(input instanceof Request ? input.url : input);
    if (url.startsWith(SERVER)) return nativeFetch(...args);
    const method = (init && init.method) || (input instanceof Request ? input.method : 'GET');
    const reqBody = await bodyText(init && init.body, input);
    if (url.startsWith(API) && typeof reqBody === 'string' && reqBody.startsWith('requests=')) {
      const headers = {};
      const h = init && init.headers ? new Headers(init.headers) : (input instanceof Request ? input.headers : new Headers());
      h.forEach((v, k) => { headers[k] = v; });
      lastApiInit = { method, headers, credentials: (init && init.credentials) || (input instanceof Request ? input.credentials : 'same-origin'), mode: init && init.mode };
    }
    const res = await nativeFetch(...args);
    // The capture pipeline reads nothing but the private API, and needs those bodies whole
    // whatever they claim to be; everything else is subject to respText.
    (url.startsWith(API) ? res.clone().text() : respText(res))
      .then((txt) => {
        if (url.startsWith(API)) inspectApiResponse(txt);
        return log('fetch', { m: method, u: url, body: reqBody, status: res.status, resp: txt });
      })
      .catch(() => {});
    return res;
  };

  // ---- manual helpers (run from the devtools console) ------------------------------------
  apiCall = async (call, params) => {
    if (!lastApiInit) throw new Error('No private API call seen yet — wait until the game has loaded.');
    const body = 'requests=' + encodeURIComponent(JSON.stringify([{ call, params }]));
    const res = await nativeFetch(API, { ...lastApiInit, method: 'POST', body });
    const json = await res.json();
    return json[call];
  };
  window.__ur = {
    apiCall,
    // Dump the site's own card database (every card at every level) to the log server,
    // which writes data/site_characters.jsonl. `since` = timestampLastUpdate (0 = everything).
    async dumpCharacters(since = 0) {
      let page = 0, total = 0;
      for (;;) {
        const r = await apiCall('characters.get', { page, timestampLastUpdate: since });
        // An error (an expired access token, most often) used to look like the last page, so
        // the dump ended part-way without a word. Stop loudly instead: a partial file must
        // never be mistaken for the whole catalog.
        if (!(r && r.data && Array.isArray(r.data.characters))) {
          throw new Error(`characters.get page ${page} failed after ${total} rows: ${JSON.stringify(r)}. ` +
            'Reload a /game/play/webgl/ tab so the API token is fresh, then run __ur.dumpCharacters() again.');
        }
        const chars = r.data.characters;
        total += chars.length;
        await log('characters', { page, since, count: chars.length, hasNextPage: !!(r && r.data && r.data.hasNextPage), characters: chars, raw: chars.length ? undefined : r });
        console.log(`characters.get page ${page}: ${chars.length} rows (total ${total})`);
        if (!(r && r.data && r.data.hasNextPage)) break;
        page++;
      }
      return total;
    },
    // Dump the clan list (names + bonuses) to the log server -> data/site_clans.json
    async dumpClans() {
      const r = await apiCall('clans.get', { timestampLastUpdate: 0 });
      const clans = (r && r.data && r.data.clans) || [];
      await log('clans', { count: clans.length, clans, raw: clans.length ? undefined : r });
      console.log(`clans.get: ${clans.length} clans`);
      return clans.length;
    },
  };

  // ---- deck apply (pure: tests/decks/Userscript.test.ts cuts this block out and runs it) ----
  // The only writes this script can make to the account. Deck Lab queues a draft in the deck
  // service (src/decks/Apply.ts); the UR Lab panel below shows it, and only the owner's click
  // on its Apply button runs applyDeckRequest. The allowlist lives here, not in anything a
  // server sends: loaddeck to read, savedeck to write, nothing else - no deleting, evolving,
  // selling or buying, and "make current" travels inside savedeck as its set_current field.
  const SITE_DECK_ACTIONS = ['loaddeck', 'savedeck'];
  const MAX_DECK_NAME = 32; // general.config.maxNameLengthDeck
  const cardKeys = (cards) => (cards || []).map((c) => `${Number(c.id)}:${Number(c.level)}:${String(c.state ?? '')}`).sort();
  // The site reorders a saved deck, so the same cards means the same multiset.
  const sameCards = (a, b) => JSON.stringify(cardKeys(a)) === JSON.stringify(cardKeys(b));
  // loaddeck and savedeck answer `Characters`; the game client's decks say `characters`.
  const deckCards = (deck) => (deck && (deck.Characters || deck.characters)) || [];
  // The request is checked again here, so the panel never sends what it has not checked itself.
  const checkApplyRequest = (req) => {
    if (!req || typeof req.id !== 'string') return 'no request';
    if (!Number.isInteger(req.deckId) || req.deckId < 0) return 'no deck id';
    const name = typeof req.name === 'string' ? req.name : '';
    if (!name.trim() || [...name].length > MAX_DECK_NAME) return `a deck name is 1 to ${MAX_DECK_NAME} characters`;
    if (typeof req.setCurrent !== 'boolean') return 'no make-current choice';
    const cards = req.characters;
    if (!Array.isArray(cards) || cards.length < 4 || cards.length > 30) return 'a deck is 4 to 30 cards';
    for (const c of cards) {
      if (!c || !Number.isInteger(c.id) || c.id <= 0 || !Number.isInteger(c.level) || c.level < 1 || c.level > 5 ||
        typeof c.state !== 'string' || !/^[a-z0-9]{0,4}$/.test(c.state)) return 'a malformed card: ' + JSON.stringify(c);
    }
    if (new Set(cards.map((c) => c.id)).size !== cards.length) return 'the same card twice';
    if (req.deckId > 0 && !(req.before && Array.isArray(req.before.characters))) return 'no record of the deck it overwrites';
    return null;
  };
  // The fields of savedeck after `action`, in the order Collection Pro's own save sends them.
  const savedeckFields = (req) => [
    ['id', String(req.deckId)],
    ['name', req.name],
    ['set_current', req.setCurrent ? 'true' : 'false'],
    ...req.characters.flatMap((c, i) => [
      [`characters[${i}][id]`, String(c.id)],
      [`characters[${i}][level]`, String(c.level)],
      [`characters[${i}][state]`, c.state],
    ]),
  ];
  // A form post to /ajax/collection/ the way the page's own jQuery makes one: same origin,
  // the session cookie, form-encoded, X-Requested-With. Refuses any action off the allowlist.
  const siteDeckPost = (fetchFn, origin) => async (action, fields) => {
    if (!SITE_DECK_ACTIONS.includes(action)) throw new Error(`UR Lab never sends ${action}`);
    const res = await fetchFn(origin + '/ajax/collection/', {
      method: 'POST',
      credentials: 'same-origin',
      headers: {
        'Content-Type': 'application/x-www-form-urlencoded; charset=UTF-8',
        'X-Requested-With': 'XMLHttpRequest',
        Accept: 'application/json, text/javascript, */*; q=0.01',
      },
      body: new URLSearchParams([['action', action], ...fields]).toString(),
    });
    const text = await res.text();
    try {
      return JSON.parse(text);
    } catch {
      return { error: `the site answered ${res.status} with something that is not JSON`, text: text.slice(0, 300) };
    }
  };
  // Overwriting: re-read the target first and stop if it is not the deck the owner confirmed
  // (the undo log's "before" must be true). Then save, read the result back and compare.
  // `sent` says whether savedeck went out, so the deck service knows the site may have changed.
  const applyDeckRequest = async (req, post) => {
    const problem = checkApplyRequest(req);
    if (problem) return { ok: false, sent: false, error: 'not sent: ' + problem };
    let sent = false;
    let saved;
    try {
      if (req.deckId > 0) {
        const current = await post('loaddeck', [['id', String(req.deckId)]]);
        if (!current || !current.deck || Number(current.deck.id) !== req.deckId) {
          return { ok: false, sent, siteResponse: current, error: `the site did not load "${req.before.name}", so nothing was saved` };
        }
        if (!sameCards(deckCards(current.deck), req.before.characters)) {
          return {
            ok: false,
            sent,
            siteResponse: current,
            error: `"${req.before.name}" changed on the site since Deck Lab last saw it, so nothing was saved. ` +
              'Deck Lab now has the site\'s version: send the draft again to see the real difference.',
          };
        }
      }
      sent = true;
      saved = await post('savedeck', savedeckFields(req));
      const id = Number(saved && saved.deck && saved.deck.id);
      if (!Number.isInteger(id) || id <= 0) {
        return { ok: false, sent, siteResponse: saved, error: 'the site did not save it: ' + JSON.stringify(saved).slice(0, 300) };
      }
      const loaded = await post('loaddeck', [['id', String(id)]]);
      const deck = (loaded && loaded.deck) || null;
      const ok = !!deck && Number(deck.id) === id && (req.deckId === 0 || id === req.deckId) && sameCards(deckCards(deck), req.characters);
      return {
        ok,
        sent,
        deck,
        siteResponse: saved,
        ...(ok ? {} : { error: deck ? 'the deck the site saved does not hold the requested cards' : 'the saved deck could not be loaded back' }),
      };
    } catch (e) {
      return { ok: false, sent, siteResponse: saved, error: String((e && e.message) || e) };
    }
  };
  // ---- end of deck apply ----

  // ---- deck panel (Collection Pro only) --------------------------------------------------
  // A side panel beside the site's own deck editor. It reads the deck being edited from the
  // page's deck list (each card link carries its id, level and edition), asks the local deck
  // service (`deno task decks`, src/decks/Service.ts, reached through the log server's
  // /decks/ so the browser needs only one local port) for a report, and shows it: legality in
  // every format as the site's own validator would judge it, stars, clans, how often each
  // card's bonus is live, full card texts and the copies owned. Every few seconds it also asks
  // whether Deck Lab has a deck waiting to be saved; if so it opens and shows what would
  // change, and saves nothing until the owner clicks Apply (see "deck apply" above).
  const DECK_SERVICE = SERVER_ROOT + '/decks';
  const APPLY_POLL_MS = 3000;
  const deckPanel = () => {
    const host = document.createElement('div');
    host.id = 'ur-lab-deck-panel';
    const root = host.attachShadow({ mode: 'open' });
    root.innerHTML = `<style>
      :host { all: initial; }
      .wrap { position: fixed; left: 10px; bottom: 10px; z-index: 2147483000; font: 12px/1.35 system-ui, sans-serif; color: #eee; }
      button.toggle { background: #f5c518; color: #111; border: 0; border-radius: 6px; padding: 6px 10px; font-weight: 700; cursor: pointer; box-shadow: 0 2px 8px #0008; }
      .panel { display: none; margin-bottom: 6px; width: 560px; max-width: calc(100vw - 20px); max-height: 78vh; overflow: auto;
        background: #16181d; border: 1px solid #444; border-radius: 8px; padding: 10px; box-shadow: 0 6px 24px #000a; }
      .open .panel { display: block; }
      h3 { margin: 0 0 6px; font-size: 14px; color: #f5c518; }
      .chips span { display: inline-block; margin: 0 6px 6px 0; padding: 2px 8px; border-radius: 10px; border: 1px solid #555; }
      .ok { color: #7ee07e; } .bad { color: #ff7b7b; } .unk { color: #e0c060; } .sel { border-color: #f5c518 !important; }
      ul.errs { margin: 0 0 8px 16px; padding: 0; } ul.errs li { color: #ff9b9b; }
      table { border-collapse: collapse; width: 100%; } td, th { padding: 3px 4px; border-top: 1px solid #333; vertical-align: top; text-align: left; }
      th { color: #aaa; font-weight: 600; } .dim { color: #999; } .ban { color: #111; background: #ff7b7b; border-radius: 3px; padding: 0 3px; margin-left: 3px; font-size: 10px; }
      .note { color: #e0c060; margin-top: 6px; }
      .apply { border: 2px solid #f5c518; border-radius: 6px; padding: 8px; margin-bottom: 10px; background: #221f12; }
      .apply ul { margin: 4px 0 6px 16px; padding: 0; } .apply .headline { font-weight: 600; margin-bottom: 4px; }
      .apply .sends { margin-top: 6px; } .apply .buttons { margin-top: 8px; display: flex; gap: 8px; }
      .apply button { border: 0; border-radius: 5px; padding: 5px 10px; font: 600 12px system-ui, sans-serif; cursor: pointer; }
      .apply button.go { background: #f5c518; color: #111; } .apply button.no { background: #333; color: #eee; }
      .apply button:disabled { opacity: 0.5; cursor: default; }
    </style><div class="wrap"><div class="panel"><div class="apply" hidden></div><div class="report"></div></div>` +
      '<button class="toggle" title="UR Lab deck report; it saves a deck from Deck Lab only when you click Apply">UR Lab ▲</button></div>';
    const wrap = root.querySelector('.wrap');
    const reportBox = root.querySelector('.report');
    const applyBox = root.querySelector('.apply');
    const toggle = root.querySelector('button.toggle');
    toggle.addEventListener('click', () => {
      wrap.classList.toggle('open');
      toggle.textContent = wrap.classList.contains('open') ? 'UR Lab ▼' : 'UR Lab ▲';
      refresh(true);
    });
    document.body.appendChild(host);

    const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);
    const readDeck = () => [...document.querySelectorAll('.js-deck-cards-list > li a.js-load-character')].map((a) => ({
      id: Number(a.dataset.characterId),
      level: Number(a.dataset.characterLevel),
      state: a.dataset.characterState || '',
    }));
    const selectedFormat = () => Number(document.querySelector('.js-deck-format-filter')?.value || 0);
    const deckName = () => document.querySelector('.js-deck-save')?.getAttribute('data-name') || '(unsaved)';

    const render = (report, deck) => {
      const selected = selectedFormat();
      const chips = report.formats.map((f) => {
        const cls = f.legal === true ? 'ok' : f.legal === false ? 'bad' : 'unk';
        const mark = f.legal === true ? '✓' : f.legal === false ? '✗' : '?';
        const why = f.errors.map((e) => e.description).concat(f.unknown.map((u) => 'not understood: ' + u.name)).join('\n');
        return `<span class="${cls}${f.formatId === selected ? ' sel' : ''}" title="${esc(why)}">${mark} ${esc(f.name)}</span>`;
      }).join('');
      const chosen = report.formats.find((f) => f.formatId === selected);
      const errors = chosen && chosen.errors.length
        ? `<ul class="errs">${chosen.errors.map((e) => `<li>${esc(e.description)}</li>`).join('')}</ul>` : '';
      const clans = report.clans.map((c) => `${esc(c.clan)} ×${c.count}`).join(', ') + (report.leaders ? `, Leaders ×${report.leaders}` : '');
      const rows = report.cards.map((c, i) => {
        if (!c.known) return `<tr><td colspan="5" class="bad">#${c.id} level ${c.level}: not in the captured card list</td></tr>`;
        const bans = [
          c.bans.tourney && 'T', c.bans.tourneyMaxLevel && c.level >= c.levelMax && 'T max',
          c.bans.elo && 'ELO', c.bans.efcMaxLevel && c.level >= c.levelMax && 'EFC max', c.bans.efcTemporary && 'EFC temp',
        ].filter(Boolean).map((b) => `<span class="ban">${b}</span>`).join('');
        const invalid = chosen && chosen.invalidIndexes.includes(i) ? ' class="bad"' : '';
        const owned = c.ownedExact === undefined ? '' : c.ownedExact > 0 ? `×${c.ownedExact}` : `<span class="bad">none${c.ownedAtLevel ? ` (${c.ownedAtLevel} other ed.)` : ''}</span>`;
        const share = c.bonusLiveShare === undefined ? '—' : Math.round(c.bonusLiveShare * 100) + '%';
        return `<tr><td${invalid}>${esc(c.name)}${bans}<div class="dim">${esc(c.clan)} · L${c.level}/${c.levelMax}${c.state ? ' · ' + esc(c.state) : ''}</div></td>` +
          `<td>${c.power}/${c.damage}</td><td>${esc(c.ability)}${c.abilityLocked && c.abilityUnlockLevel ? `<div class="dim">unlocks at L${c.abilityUnlockLevel}</div>` : ''}</td>` +
          `<td>${esc(c.bonus)}<div class="dim">live ${share}</div></td><td>${owned}</td></tr>`;
      }).join('');
      const cap = chosen?.maxStars ? '/' + chosen.maxStars : '';
      reportBox.innerHTML = `<h3>${esc(deckName())} · ${deck.length} cards · ${report.stars}${cap}★</h3>` +
        `<div class="chips">${chips}</div>${errors}<div class="dim" style="margin-bottom:6px">${clans}</div>` +
        `<table><tr><th>Card</th><th>P/D</th><th>Ability</th><th>Bonus</th><th>Owned</th></tr>${rows}</table>` +
        report.notes.map((n) => `<div class="note">${esc(n)}</div>`).join('');
    };

    let lastKey = '';
    let timer = 0;
    const refresh = async (force = false) => {
      if (!wrap.classList.contains('open')) return;
      const deck = readDeck();
      const key = JSON.stringify([deck, selectedFormat(), !!window.isNight]);
      if (!force && key === lastKey) return;
      lastKey = key;
      if (!deck.length) { reportBox.innerHTML = '<h3>UR Lab</h3><div class="dim">Load or build a deck to see its report.</div>'; return; }
      try {
        const res = await nativeFetch(DECK_SERVICE + '/api/report', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ characters: deck, night: !!window.isNight }),
        });
        const report = await res.json();
        if (!res.ok) throw new Error(report.error || res.status);
        render(report, deck);
      } catch (e) {
        reportBox.innerHTML = `<h3>UR Lab</h3><div class="unk">No deck report: run <b>deno task decks</b> (and the log server) in the repository (${esc(e && e.message)}).</div>`;
      }
    };
    // The site rebuilds the deck list as cards are added, removed or re-levelled; watch the
    // whole page cheaply and only ask again when the deck or the chosen room changed.
    new MutationObserver(() => { clearTimeout(timer); timer = setTimeout(() => refresh(), 250); })
      .observe(document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ['data-character-level', 'data-character-state'] });
    document.addEventListener('change', (e) => { if (e.target?.classList?.contains('js-deck-format-filter')) refresh(true); }, true);

    // A deck Deck Lab asks to save. The apply area shows nothing, a pending request (Apply and
    // Dismiss), "saving", or the outcome, which stays until closed or another request comes.
    let box = { phase: 'none', id: null };
    const localJson = async (path, init) => {
      const res = await nativeFetch(DECK_SERVICE + path, { cache: 'no-store', ...init });
      return { ok: res.ok, status: res.status, body: await res.json().catch(() => null) };
    };
    // The latest request in any state, null when there is none; throws when the deck service
    // (or the log server in front of it) is not there.
    const latestRequest = async () => {
      const { ok, status, body } = await localJson('/api/apply');
      if (!ok) throw new Error(`the deck service answered ${status}`);
      return body && body.id ? body : null;
    };
    const step = (req, name, body) => localJson(`/api/apply/${encodeURIComponent(req.id)}/${name}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
    const report = (req, outcome) => step(req, 'result', outcome);
    const openPanel = () => { if (!wrap.classList.contains('open')) toggle.click(); };
    const cardText = (c, names) => `${names.get(c.id) || '#' + c.id} L${c.level}${c.state ? ' · ' + c.state : ''}`;
    const showOutcome = (id, html, reload = false) => {
      box = { phase: 'outcome', id };
      applyBox.innerHTML = html + '<div class="buttons">' +
        (reload ? '<button class="go">Reload the page</button>' : '') + '<button class="no">Close</button></div>';
      applyBox.hidden = false;
      applyBox.querySelector('.go')?.addEventListener('click', () => location.reload());
      applyBox.querySelector('.no').addEventListener('click', () => { applyBox.hidden = true; box = { phase: 'none', id }; });
    };
    const showPending = (req) => {
      box = { phase: 'pending', id: req.id };
      const s = req.summary || {};
      const problem = checkApplyRequest(req);
      // What goes out is listed from the very fields savedeck will carry.
      const names = new Map((s.cards || []).map((c) => [c.id, c.name]));
      const target = req.before ? req.before.name : '#' + req.deckId;
      applyBox.innerHTML = '<h3>Deck Lab asks to save a deck</h3>' +
        `<div class="headline">${esc(s.headline)}</div>` +
        (s.lines && s.lines.length ? `<ul>${s.lines.map((l) => `<li>${esc(l)}</li>`).join('')}</ul>` : '') +
        (s.warnings || []).map((w) => `<div class="unk">${esc(w)}</div>`).join('') +
        (problem
          ? `<div class="bad">UR Lab will not send this: ${esc(problem)}</div>`
          : `<div class="dim sends">Sends savedeck with id ${req.deckId}${req.deckId ? '' : ' (a new deck)'}, name "${esc(req.name)}", ` +
            `make current ${req.setCurrent ? 'yes' : 'no'} and ${req.characters.length} cards: ` +
            `${esc(req.characters.map((c) => cardText(c, names)).join(', '))}. Nothing is sent until you click.</div>`) +
        '<div class="buttons">' +
        (problem ? '' : `<button class="go">${esc(req.deckId ? `Overwrite "${target}"` : `Create "${req.name}"`)}</button>`) +
        '<button class="no">Dismiss</button></div>';
      applyBox.hidden = false;
      applyBox.querySelector('.go')?.addEventListener('click', () => apply(req));
      applyBox.querySelector('.no').addEventListener('click', () => dismiss(req));
      openPanel();
    };
    const apply = async (req) => {
      if (box.phase !== 'pending' || box.id !== req.id) return;
      box = { phase: 'busy', id: req.id };
      applyBox.querySelectorAll('button').forEach((b) => { b.disabled = true; });
      applyBox.querySelector('.go').textContent = 'Saving…';
      // Take the request first: only one tab can, and Deck Lab can no longer withdraw or
      // replace it underneath. Deck Lab may already have done so, which ends it here.
      let claim;
      try {
        claim = await step(req, 'claim', {});
      } catch (e) {
        showOutcome(req.id, `<div class="bad">Nothing was sent: ${esc(e && e.message)} (is <b>deno task decks</b> running?).</div>`);
        return;
      }
      if (!claim.ok) {
        const why = (claim.body && claim.body.error) || `the deck service answered ${claim.status}`;
        showOutcome(req.id, `<div class="unk">Nothing was sent: ${esc(why)}. Deck Lab withdrew or replaced it, or another tab took it.</div>`);
        return;
      }
      // The page's own (patched) fetch, not nativeFetch, so these calls reach the log like the site's.
      const outcome = await applyDeckRequest(req, siteDeckPost((...a) => globalThis.fetch(...a), location.origin));
      let recorded = false;
      try {
        recorded = (await report(req, outcome)).ok;
      } catch { /* said below */ }
      let html;
      if (outcome.ok) {
        const made = req.setCurrent ? (outcome.deck.isCurrent ? ', and it is your current deck' : '; the site does not list it as your current deck, though') : '';
        html = `<div class="ok">Saved "${esc(outcome.deck.name || req.name)}" and read it back: the site's deck holds exactly these ` +
          `${req.characters.length} cards${made}.</div><div>Reload this page so the site's own deck list shows it.</div>`;
      } else if (!outcome.sent) {
        html = `<div class="bad">Nothing was saved: ${esc(outcome.error)}</div>`;
      } else {
        html = `<div class="bad">The site was asked to save it, but ${esc(outcome.error)}.</div>` +
          '<div>Reload this page and check the deck in the site\'s own list.</div>';
      }
      if (!recorded && outcome.sent) {
        console.warn('UR Lab: a deck write the deck service did not record', { request: req, outcome });
        html += '<div class="unk">The deck service did not record this, so data/deck_history.jsonl has no entry for it. ' +
          `The deck held: ${esc(req.before ? req.before.characters.map((c) => cardText(c, new Map())).join(', ') : 'nothing (a new deck)')}.</div>`;
      }
      showOutcome(req.id, html, outcome.sent);
    };
    const dismiss = async (req) => {
      box = { phase: 'none', id: req.id };
      applyBox.hidden = true;
      try {
        await report(req, { dismissed: true });
      } catch { /* it stays dismissed on this page all the same */ }
    };
    const ENDED = {
      applying: 'another tab is saving it',
      applied: 'another tab saved it',
      failed: 'another tab tried to save it and failed',
      dismissed: 'it was dismissed in another tab',
      discarded: 'Deck Lab withdrew or replaced it',
    };
    const pollApply = async () => {
      if (box.phase === 'busy') return;
      let req;
      try {
        req = await latestRequest();
      } catch {
        return; // the log server or the deck service is not running: leave the panel as it is
      }
      if (box.phase === 'busy') return;
      if (req && req.state === 'pending' && req.id !== box.id) showPending(req);
      else if (box.phase === 'pending' && !(req && req.id === box.id && req.state === 'pending')) {
        const why = req && req.id === box.id ? ENDED[req.state] || req.state : 'Deck Lab withdrew it';
        showOutcome(box.id, `<div class="dim">This request is over: ${esc(why)}. Nothing was sent from this tab.</div>`);
      }
    };
    pollApply();
    setInterval(pollApply, APPLY_POLL_MS);
  };
  // ---- game in the whole window (the game page only) --------------------------------------
  // The site's fullscreen button takes the whole monitor. This fills just the browser window:
  // the Unity canvas is fixed over the page at the window's size, and Unity, whose render size
  // follows the canvas's DOM size, re-renders to fit. Everything else on the page is hidden
  // meanwhile; the canvas's own siblings (Unity's loading bar and warnings) are left alone. A
  // small button in the bottom-right corner switches it, and this browser remembers the choice.
  const THEATER_KEY = 'ur-theater';
  const theaterMode = () => {
    const style = document.createElement('style');
    style.textContent = `
      html.ur-theater, html.ur-theater body { overflow: hidden !important; }
      html.ur-theater #unity-canvas { position: fixed !important; inset: 0 !important; width: 100vw !important;
        height: 100vh !important; max-width: none !important; max-height: none !important; margin: 0 !important;
        border: 0 !important; z-index: 2147482000 !important; background: #000; }
      html.ur-theater .ur-theater-path { transform: none !important; filter: none !important; contain: none !important;
        will-change: auto !important; }
      html.ur-theater .ur-theater-hidden { visibility: hidden !important; }`;
    document.head.appendChild(style);
    const host = document.createElement('div');
    host.id = 'ur-theater-toggle';
    const root = host.attachShadow({ mode: 'open' });
    root.innerHTML = `<style>
      :host { all: initial; }
      button { position: fixed; right: 6px; bottom: 6px; z-index: 2147483000; width: 26px; height: 26px; padding: 0;
        border: 0; border-radius: 5px; background: #f5c518; color: #111; font: 700 15px/26px system-ui, sans-serif;
        cursor: pointer; opacity: 0.35; box-shadow: 0 2px 6px #0008; }
      button:hover { opacity: 1; }
    </style><button title="Fill the window with the game (UR logger)">⛶</button>`;
    const button = root.querySelector('button');
    // Outside <body>, so hiding the rest of the page never hides the button itself.
    document.documentElement.appendChild(host);

    const remembered = () => {
      try {
        return localStorage.getItem(THEATER_KEY) === 'on';
      } catch {
        return false;
      }
    };
    const apply = (on) => {
      document.querySelectorAll('.ur-theater-path, .ur-theater-hidden')
        .forEach((e) => e.classList.remove('ur-theater-path', 'ur-theater-hidden'));
      const canvas = document.getElementById('unity-canvas');
      document.documentElement.classList.toggle('ur-theater', on && !!canvas);
      if (on && canvas) {
        for (let e = canvas.parentElement; e && e !== document.body && e !== document.documentElement; e = e.parentElement) {
          e.classList.add('ur-theater-path');
          for (const sibling of e.parentElement?.children ?? []) {
            if (sibling !== e && !sibling.id?.startsWith('ur-') && sibling.tagName !== 'SCRIPT') {
              sibling.classList.add('ur-theater-hidden');
            }
          }
        }
      }
      button.textContent = on ? '✕' : '⛶';
      button.title = on ? 'Back to the normal page (UR logger)' : 'Fill the window with the game (UR logger)';
      // Unity checks the canvas size every frame; a resize event makes it happen at once.
      window.dispatchEvent(new Event('resize'));
    };
    button.addEventListener('click', () => {
      const on = !document.documentElement.classList.contains('ur-theater');
      try {
        localStorage.setItem(THEATER_KEY, on ? 'on' : 'off');
      } catch { /* the choice just is not remembered */ }
      apply(on);
    });
    if (remembered()) {
      // The canvas exists from the start, but let the site's own layout script run first.
      let tries = 0;
      const wait = setInterval(() => {
        if (document.getElementById('unity-canvas') || ++tries > 120) {
          clearInterval(wait);
          apply(true);
        }
      }, 500);
    }
  };
  if (location.pathname.startsWith('/game/play')) {
    if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', theaterMode);
    else theaterMode();
  }

  if (location.pathname.startsWith('/collection/pro')) {
    if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', deckPanel);
    else deckPanel();
  }

  log('page', { href: location.href, ua: navigator.userAgent, version: VERSION });
})();
