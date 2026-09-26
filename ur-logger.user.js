// ==UserScript==
// @name         UR logger
// @namespace    urban-recreation
// @version      0.7.1
// @description  Mirror Urban Rivals network traffic to a local log server (see log_server.ts)
// @match        https://www.urban-rivals.com/*
// @run-at       document-start
// @grant        none
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

  log('page', { href: location.href, ua: navigator.userAgent });
})();
