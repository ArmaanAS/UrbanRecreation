// ==UserScript==
// @name         UR logger
// @namespace    urban-recreation
// @version      0.4
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
//   as 'b64:<base64>' so nothing is lost.
(() => {
  const SERVER = 'http://localhost:8787/log';
  const nativeFetch = window.fetch.bind(window); // saved BEFORE we patch anything

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

  // ---- WebSocket: both directions -----------------------------------------------------
  const NativeWS = window.WebSocket;
  window.WebSocket = function (url, protocols) {
    const ws = protocols ? new NativeWS(url, protocols) : new NativeWS(url);
    log('ws_open', String(url));
    ws.addEventListener('message', (e) => {
      if (typeof e.data === 'string') log('ws_in', e.data);
      else if (e.data instanceof ArrayBuffer) log('ws_in', decodeBytes(new Uint8Array(e.data)));
      else if (e.data instanceof Blob) e.data.arrayBuffer().then((ab) => log('ws_in', decodeBytes(new Uint8Array(ab))));
      else log('ws_in', '[binary]');
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
        resp = this.responseType === '' || this.responseType === 'text' ? this.responseText
          : this.responseType === 'json' ? JSON.stringify(this.response)
          : this.responseType === 'arraybuffer' ? decodeBytes(new Uint8Array(this.response))
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
    res.clone().text()
      .then((txt) => log('fetch', { m: method, u: url, body: reqBody, status: res.status, resp: txt }))
      .catch(() => {});
    return res;
  };

  // ---- manual helpers (run from the devtools console) ------------------------------------
  const apiCall = async (call, params) => {
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
        const chars = (r && r.data && r.data.characters) || [];
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
