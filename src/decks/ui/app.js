// Deck Lab: browse the collection, draft a deck and see its report, all from the data the
// log server captured while Collection Pro was open. Served by src/decks/Service.ts. Nothing
// here talks to urban-rivals.com: "Send to site" only queues the draft in the deck service,
// and the userscript's UR Lab panel on Collection Pro saves it after the owner clicks there.
const $ = (id) => document.getElementById(id);
const esc = (s) =>
  String(s ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
const PAGE = 200;
const STORE_KEY = "deck-lab-draft";

const state = {
  cards: [],
  byId: new Map(),
  formats: [],
  decks: [],
  legalByFormat: {},
  /** Opposing hands seen in the chosen format: card id -> hands it appeared in. */
  meta: { hands: 0, byId: new Map(), clans: [] },
  shown: PAGE,
  draft: { name: "", sourceId: 0, cards: [] },
  /** Which cards the exact solver can score (GET /api/coverage), or null before card-coverage ran. */
  coverage: null,
  /** The last scoring job (GET /api/matchup). */
  matchup: null,
  matchupTimer: 0,
  /** Decks improved by `deno task deck-search` (GET /api/suggestions). */
  suggestions: [],
  /** Deck writes made through the UR Lab panel, newest first (GET /api/apply/history). */
  history: [],
};

// ---- per-viewer draft persistence (a convenience: the page works without it) -------------
function saveDraft() {
  try {
    localStorage.setItem(STORE_KEY, JSON.stringify(state.draft));
  } catch { /* private window or blocked storage */ }
}
function loadDraft() {
  try {
    const saved = JSON.parse(localStorage.getItem(STORE_KEY) ?? "null");
    if (saved && Array.isArray(saved.cards)) state.draft = saved;
  } catch { /* nothing saved */ }
}

// ---- helpers ----------------------------------------------------------------------------
const format = () => state.formats.find((f) => f.id === Number($("format").value));
const night = () => $("night").checked;
const ownedLevels = (card) => Object.keys(card.owned).map(Number).filter((l) => card.evos[l]);

function displayLevel(card) {
  const mode = $("levelMode").value;
  const owned = ownedLevels(card);
  if (mode === "owned" && owned.length) return Math.max(...owned);
  if (mode === "min") return card.levelMin;
  return card.levelMax;
}

function abilityAt(card, level) {
  const [, , ability, unlock, nightAbility] = card.evos[level];
  const text = night() && nightAbility ? nightAbility : ability;
  return unlock > level ? `No Ability (unlocks at L${unlock})` : text;
}

/** The criteria a single card can break on its own, for the "legal in format" filter. */
function cardProblems(card, level, f) {
  if (!f) return [];
  const problems = [];
  for (const c of f.criteria) {
    if (c.name === "forbidden_character_list" && c.value.includes(card.id)) problems.push("banned");
    if (c.name === "forbidden_maxed_character_list" && level >= card.levelMax && c.value.includes(card.id)) {
      problems.push("banned at max level");
    }
    if (c.name === "exclude_elo_forbidden" && card.bans.elo) problems.push("ELO ban");
    const lv = /^max_level(\d)_characters$/.exec(c.name);
    if (lv && Number(lv[1]) === level && c.value === 0) problems.push(`no level-${level} cards`);
    if (c.name === "no_collectors" && card.rarity === "cr") problems.push("no collectors");
  }
  return problems;
}

// ---- solver coverage: which cards the exact solver can score ------------------------------
const COVERAGE_WORDS = {
  b: "its clan bonus is not modelled",
  u: "its clan bonus could not be tested",
  r: "not modelled",
  l: "Leaders are not modelled",
  m: "not in the engine's card data",
};

/** `{exact, code, why}` for a card at a level at the current time of day, or null if unknown. */
function solverStatus(card, level) {
  const row = state.coverage?.cards?.[card.id]?.[level];
  const t = night() ? 1 : 0;
  const code = row?.[t];
  if (!code) return null;
  const why = row[2 + t] >= 0 ? state.coverage.reasons[row[2 + t]] : COVERAGE_WORDS[code];
  return { exact: code === "e" || code === "u", code, why: code === "b" ? `${why} (it scores only without a clan-mate)` : why };
}

function solverBadge(card, level) {
  const s = solverStatus(card, level);
  if (!s || s.exact) return "";
  const title = `The exact solver cannot score this card ${night() ? "at night" : "by day"}: ${s.why}`;
  return `<span class="badge solver" title="${esc(title)}">${s.code === "b" ? "solver: alone" : "no solver"}</span>`;
}

function renderCoverageLine() {
  const el = $("coverageLine");
  if (!state.coverage) {
    el.innerHTML = "Run <code>deno task rust:matchup</code> and <code>deno task card-coverage</code> to see which cards the solver can score.";
    return;
  }
  const cards = state.draft.cards.map((c) => ({ c, card: state.byId.get(c.id) })).filter((x) => x.card);
  const out = cards.map(({ c, card }) => ({ c, card, s: solverStatus(card, c.level) })).filter(({ s }) => s && !s.exact);
  let html = cards.length
    ? `The solver can score ${cards.length - out.length} of ${cards.length} cards ${night() ? "at night" : "by day"}` +
      (out.length ? "; hand pairs holding the others are not scored:" : ".")
    : "";
  if (out.length) {
    html += `<ul>${out.map(({ c, card, s }) => `<li>${esc(card.name)} L${c.level}: ${esc(s.why)}</li>`).join("")}</ul>`;
  }
  if (state.coverage.stale) {
    html += `<div class="note">Solver coverage may be out of date: ${esc(state.coverage.stale)}. Rerun <code>deno task card-coverage</code>.</div>`;
  }
  el.innerHTML = html;
}

// ---- scoring the draft on the exact solver ------------------------------------------------
function renderOpponents() {
  const f = format();
  const keep = $("opponent").value;
  const field = f && state.meta.hands
    ? `<option value="field">Captured ${esc(f.name)} opponents (${state.meta.hands} hands)</option>`
    : "";
  $("opponent").innerHTML = field +
    state.decks.map((d) => `<option value="deck:${d.id}">Deck: ${esc(d.name)} · ${d.characters.length}</option>`).join("");
  if ([...$("opponent").options].some((o) => o.value === keep)) $("opponent").value = keep;
}

/** Scores the draft, or with `slot` looks for a better owned card for that slot. */
async function startScore(slot) {
  const choice = $("opponent").value;
  if (!choice) return;
  const opponent = choice === "field" ? { format: format()?.id } : { deck: Number(choice.slice("deck:".length)) };
  const swap = Number.isInteger(slot) ? { slot, scope: $("swapScope").value, keepAll: $("keepAll").checked } : undefined;
  try {
    const res = await fetch("/api/matchup", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        characters: state.draft.cards,
        night: night(),
        n: Number($("hands").value),
        opponent,
        format: format()?.id,
        swap,
      }),
    });
    const body = await res.json();
    if (!res.ok) throw new Error(body.error ?? res.status);
    state.matchup = body;
  } catch (e) {
    $("scoreOut").innerHTML = `<div class="note">Cannot score: ${esc(e.message)}</div>`;
    return;
  }
  renderScore();
  pollScore();
}

function pollScore() {
  clearTimeout(state.matchupTimer);
  if (state.matchup?.state !== "running") return;
  state.matchupTimer = setTimeout(async () => {
    try {
      state.matchup = await fetch("/api/matchup").then((r) => r.json());
    } catch { /* the service is restarting; try again */ }
    renderScore();
    pollScore();
  }, 800);
}

const pct = (x) => `${x.toFixed(1)}%`;
const cardName = ([id, level]) => `${esc(state.byId.get(id)?.name ?? `#${id}`)} L${level}`;
const handText = (hand) => hand.map(cardName).join(", ");

function renderScore() {
  const job = state.matchup;
  const out = $("scoreOut");
  const running = job?.state === "running";
  $("score").disabled = running || state.draft.cards.length < 4;
  $("cancel").hidden = !running;
  if (!job || job.state === "none") {
    out.innerHTML = "";
    return;
  }
  const when = job.night ? " at night" : "";
  const slotCard = job.kind === "swap" ? job.deck[job.slot] : null;
  if (running) {
    const what = slotCard ? `Looking for a better card than ${cardName(slotCard)}` : "Scoring";
    out.innerHTML = `<div>${what} against ${esc(job.against)}${when}…</div>` + (job.total
      ? `<progress max="${job.total}" value="${job.done}"></progress> <span class="dim">${job.done} of ${job.total} solves</span>`
      : '<span class="dim">starting the solver…</span>');
    return;
  }
  if (job.state === "failed") {
    out.innerHTML = `<div class="note">Scoring failed: ${esc(job.error)}</div>`;
    return;
  }
  if (job.state === "cancelled") {
    out.innerHTML = '<div class="dim">Stopped. The solves finished so far are cached, so scoring again picks up from there.</div>';
    return;
  }
  const r = job.result;
  const stale = JSON.stringify(job.deck) !== JSON.stringify(state.draft.cards.map((c) => [c.id, c.level]));
  const changed = stale ? '<div class="dim">The draft has changed since this score.</div>' : "";
  if (slotCard) {
    out.innerHTML = renderSwap(job, slotCard, stale) + changed;
    return;
  }
  const margin = r.stderr == null ? "" : ` <span class="dim">± ${(r.stderr * 50).toFixed(1)}</span>`;
  const head = r.scored ? `<div class="score">${pct(r.percent)}${margin}</div>` : '<div class="score bad">Nothing could be scored</div>';
  const clans = (r.byClan ?? []).filter((c) => c.scored >= 2);
  const byClan = clans.length
    ? `<div class="byclan">By opposing clan: ${
      clans.map((c) => `<span style="${heat(c.mean)}" title="${c.scored} hand pairs">${esc(c.clan)} ${pctOf(c.mean)}%</span>`)
        .join(" ")
    }</div>`
    : "";
  const worst = r.worst.length
    ? `<details><summary>Worst hands</summary><ul>${
      r.worst.map((p) => `<li><b>${pct((p.score + 1) * 50)}</b> ${handText(p.a)} <span class="dim">vs</span> ${handText(p.b)}</li>`)
        .join("")
    }</ul></details>`
    : "";
  const refusals = r.refused
    ? `<details><summary>Why ${r.refused} hand pairs were not scored</summary><ul>${
      r.refusals.map((x) =>
        `<li>${x.count}× ${x.card ? `${x.side === "a" ? "your" : "their"} ${cardName(x.card)}: ` : ""}${esc(x.why)}</li>`
      ).join("")
    }</ul></details>`
    : "";
  out.innerHTML = head + changed +
    `<div class="dim">Against ${esc(job.against)}${when}: ${r.scored} of ${r.pairs} hand pairs scored` +
    `${r.refused ? `, ${r.refused} not` : ""} · ${job.seconds.toFixed(1)} s (${r.solved} solved, ${r.cached} from the cache)</div>` +
    byClan + worst + refusals +
    '<div class="dim small">Each hand pair is solved exactly with both first movers, playing the advisor\'s conservative ' +
    "policy, which never relies on guessing hidden pillz. 50% is even. The opposing hands are the same every time, so " +
    "use it to compare drafts; it is not a win rate.</div>";
}

function renderSwap(job, slotCard, stale) {
  const r = job.result;
  const pp = (x) => `${x >= 0 ? "+" : "−"}${Math.abs(x * 50).toFixed(1)}`;
  const rows = r.candidates.map((c, i) => {
    const known = c.diff != null;
    // Pairs the draft itself loses to refusals are not the candidate's doing.
    const extra = c.refused - r.base.refused;
    const partly = extra > 0 ? ` title="${extra} more hand pairs refused than with the draft's own card"` : "";
    return `<tr><td>${cardName([c.card.id, c.card.level])}</td>` +
      `<td style="${known ? heat(c.diff * 5) : ""}"${partly}>${known ? pp(c.diff) : "?"}` +
      `${known && c.diffErr != null ? ` <span class="dim">±${(c.diffErr * 50).toFixed(1)}</span>` : ""}${extra > 0 ? " *" : ""}</td>` +
      `<td>${c.mean == null ? "" : pct((c.mean + 1) * 50)}</td>` +
      `<td>${stale ? "" : `<button class="icon" data-use="${i}" title="Put it in the draft">Use</button>`}</td></tr>`;
  }).join("");
  const base = r.base.scored ? pct((r.base.mean + 1) * 50) : "not scored";
  return `<div>Better cards than <b>${cardName(slotCard)}</b> against ${esc(job.against)}${job.night ? " at night" : ""}: ` +
    `the draft scores ${base} as it is.</div>` +
    (r.candidates.length
      ? `<table class="swap"><tr><th>Instead</th><th title="Points on the percent scale, same opposing hands">Change</th><th>Draft</th><th></th></tr>${rows}</table>`
      : '<div class="note">No owned card of that scope can take this slot.</div>') +
    `<div class="dim">${r.considered} owned cards tried (the ${r.candidates.length} best shown) · ${job.seconds.toFixed(1)} s ` +
    `(${r.solved} solved, ${r.cached} from the cache). Only the hands that draw this slot change, and against the same ` +
    "opposing hands, which keeps the ± small; * marks cards that get more hand pairs refused than the draft's own.</div>";
}

// ---- clans against each other (deno task clan-matrix) -------------------------------------
const pctOf = (mean) => (mean == null ? "" : ((mean + 1) * 50).toFixed(0));
/** Red below 50%, green above, for the dark theme. */
const heat = (mean) => {
  if (mean == null) return "";
  const t = Math.max(-1, Math.min(1, mean / 0.4));
  return `background: hsl(${60 + 60 * t}, 45%, ${16 + 10 * Math.abs(t)}%)`;
};

async function renderClans() {
  const el = $("clans");
  if (el.hidden) return;
  const f = format();
  let data = null;
  try {
    data = await fetch(`/api/clans?format=${f?.id}`).then((r) => r.json());
  } catch { /* shown as missing */ }
  const m = data?.[night() ? "night" : "day"];
  if (!m) {
    el.innerHTML = `<h3>Clans in ${esc(f?.name)}, ${night() ? "night" : "day"}</h3>` +
      `<div class="dim">Not computed yet: run <code>deno task clan-matrix --format ${esc(f?.name)}${night() ? " --night" : ""}</code>` +
      " (about half an hour for Tourney).</div>";
    return;
  }
  const order = m.clans.map((c) => c.clan);
  const cell = new Map();
  for (const c of m.cells) {
    cell.set(`${c.a}|${c.b}`, { mean: c.mean, stderr: c.stderr, scored: c.scored, refused: c.refused, practice: c.practice });
    cell.set(`${c.b}|${c.a}`, {
      mean: c.mean == null ? null : -c.mean,
      stderr: c.stderr,
      scored: c.scored,
      refused: c.refused,
      practice: { games: c.practice.games, score: c.practice.games - c.practice.score },
    });
  }
  const err = (x) => (x == null ? "" : `±${(x * 50).toFixed(1)}`);
  const ranking = `<table><tr><th class="clan">Clan</th><th title="Each clan weighted by how often your opponents play it">vs field</th>` +
    `<th title="Every other clan weighted equally">vs clans</th><th>Hands</th><th title="Share of its hand pairs the exact engine could solve; the rest are left out">Solved</th>` +
    `<th title="Captured games against the other clans here">In practice</th></tr>` +
    m.clans.map((c) =>
      `<tr><th class="clan" data-clan="${esc(c.clan)}">${esc(c.clan)}</th>` +
      `<td style="${heat(c.vsField)}">${c.vsField == null ? "–" : `${pctOf(c.vsField)}%`} <span class="dim">${err(c.vsFieldErr)}</span></td>` +
      `<td style="${heat(c.vsClans)}">${c.vsClans == null ? "–" : `${pctOf(c.vsClans)}%`} <span class="dim">${err(c.vsClansErr)}</span></td>` +
      `<td title="${c.ownerHands} of them yours, from ${c.players} players">${c.hands}</td>` +
      `<td>${c.scored + c.refused ? Math.round((100 * c.scored) / (c.scored + c.refused)) : 0}%</td>` +
      `<td>${c.practice.games ? `${c.practice.score}/${c.practice.games}` : ""}</td></tr>`
    ).join("") + "</table>";
  const matrix = `<table class="matrix"><tr><th></th>${order.map((c) => `<th class="col">${esc(c)}</th>`).join("")}</tr>` +
    order.map((row) =>
      `<tr><th class="clan" data-clan="${esc(row)}">${esc(row)}</th>` + order.map((col) => {
        if (row === col) return '<td class="self"></td>';
        const c = cell.get(`${row}|${col}`);
        if (!c) return "<td></td>";
        const title = `${row} against ${col}: ${pctOf(c.mean)}% ${err(c.stderr)} over ${c.scored} hand pairs` +
          (c.refused ? `, ${c.refused} refused` : "") +
          (c.practice.games ? `; in practice ${c.practice.score} of ${c.practice.games} captured games` : "");
        return `<td style="${heat(c.mean)}" title="${esc(title)}">${pctOf(c.mean)}</td>`;
      }).join("") + "</tr>"
    ).join("") + "</table>";
  el.innerHTML = `<h3>Clans in ${esc(m.format.name)}, ${m.night ? "night" : "day"}: each row's score against each column</h3>` +
    `<div class="cols"><div>${ranking}</div><div>${matrix}</div></div>` +
    `<div class="dim small">Exact solves of ${m.perCell} hand pairs per clan pair, drawn from the ${m.handsCaptured} ` +
    `clan hands (3+ cards of one clan) in ${m.games} captured games, both first movers, the advisor's conservative policy; ` +
    `50% is even. Most hands of the clans you play are yours. Computed ${esc(m.generatedAt.slice(0, 16).replace("T", " "))} UTC. ` +
    "Click a clan to browse its cards.</div>";
}

// ---- collection -------------------------------------------------------------------------
function filteredCards() {
  const q = $("search").value.trim().toLowerCase();
  const clan = $("clan").value;
  const rarity = $("rarity").value;
  const ownedOnly = $("ownedOnly").checked;
  const legalOnly = $("legalOnly").checked;
  const solverOnly = $("solverOnly").checked;
  const f = format();
  const rows = [];
  for (const card of state.cards) {
    if (clan && String(card.clanId) !== clan) continue;
    if (rarity && card.rarity !== rarity) continue;
    if (ownedOnly && !ownedLevels(card).length) continue;
    const level = displayLevel(card);
    if (!card.evos[level]) continue;
    const problems = cardProblems(card, level, f);
    if (legalOnly && problems.length) continue;
    if (solverOnly && solverStatus(card, level)?.exact === false) continue;
    if (q) {
      const hay = `${card.name} ${card.clan} ${abilityAt(card, level)} ${card.bonus}`.toLowerCase();
      if (!hay.includes(q)) continue;
    }
    rows.push({ card, level, problems });
  }
  const sort = $("sort").value;
  const key = {
    name: (r) => r.card.name.toLowerCase(),
    power: (r) => -r.card.evos[r.level][0],
    damage: (r) => -r.card.evos[r.level][1],
    stars: (r) => -r.level,
    release: (r) => -r.card.release,
    meta: (r) => -(state.meta.byId.get(r.card.id)?.count ?? 0),
  }[sort];
  rows.sort((a, b) => (key(a) < key(b) ? -1 : key(a) > key(b) ? 1 : a.card.name.localeCompare(b.card.name)));
  return rows;
}

function renderCollection() {
  const rows = filteredCards();
  $("count").textContent = `${rows.length} cards`;
  $("cards").innerHTML = rows.slice(0, state.shown).map(({ card, level, problems }) => {
    const [power, damage, , , , picture] = card.evos[level];
    const levels = Object.keys(card.evos).map((l) =>
      `<span class="${card.owned[l] ? "owned" : ""}" title="${esc(Object.entries(card.owned[l] ?? {})
        .map(([s, n]) => `${n}× ${s || "classic"}`).join(", ") || "not owned")}">${l}</span>`
    ).join("");
    const badges = [
      card.bans.tourney && "T",
      card.bans.tourneyMaxLevel && "T max",
      card.bans.elo && "ELO",
      card.bans.efcMaxLevel && "EFC max",
      card.bans.efcTemporary && "EFC temp",
    ].filter(Boolean).map((b) => `<span class="badge">${b}</span>`).join("");
    return `<div class="card${problems.length ? " illegal" : ""}" title="${esc(problems.join(", "))}">` +
      `${picture ? `<img loading="lazy" src="${esc(picture)}" alt="">` : "<span></span>"}` +
      `<div><div class="name">${esc(card.name)}${badges}${solverBadge(card, level)}</div><div class="dim">${esc(card.clan)} · ${esc(card.rarity)}</div>` +
      `<div class="levels">${levels}</div>${seenIn(card)}</div>` +
      `<div class="pd">L${level}<br>${power}/${damage}</div>` +
      `<div>${esc(abilityAt(card, level))}</div>` +
      `<div class="dim">${esc(night() && card.nightBonus ? card.nightBonus : card.bonus)}</div>` +
      `<button class="icon" data-add="${card.id}" data-level="${level}" title="Add to the draft">+</button></div>`;
  }).join("");
  $("more").hidden = rows.length <= state.shown;
}

// ---- meta: what opponents play in the chosen format --------------------------------------
function seenIn(card) {
  const seen = state.meta.byId.get(card.id);
  return seen ? `<div class="dim" title="levels ${esc(Object.entries(seen.levels).map(([l, n]) => `L${l}×${n}`).join(", "))}">` +
    `seen in ${seen.count} of ${state.meta.hands} opposing hands</div>` : "";
}

async function loadMeta() {
  const f = format();
  if (!f) return;
  try {
    const meta = await fetch(`/api/meta?format=${f.id}`).then((r) => r.json());
    state.meta = { hands: meta.hands, byId: new Map(meta.cards.map((c) => [c.id, c])), clans: meta.clans };
    const top = meta.clans.slice(0, 6).map((c) => `${esc(c.clan)} ${Math.round((100 * c.count) / (4 * meta.hands))}%`).join(", ");
    $("meta").innerHTML = meta.hands
      ? `${esc(f.name)} opponents in the captures: ${meta.hands} hands (${esc(meta.from?.slice(0, 10))} to ${
        esc(meta.to?.slice(0, 10))
      }). Most played clans: ${top}.`
      : `No captured ${esc(f.name)} games yet.`;
  } catch {
    state.meta = { hands: 0, byId: new Map(), clans: [] };
    $("meta").textContent = "";
  }
}

// ---- draft ------------------------------------------------------------------------------
function bestState(card, level) {
  const copies = card.owned[level] ?? {};
  return Object.keys(copies).find((s) => s === "") ?? Object.keys(copies)[0] ?? "";
}

function addToDraft(id, level) {
  const card = state.byId.get(id);
  if (!card || state.draft.cards.some((c) => c.id === id)) return;
  state.draft.cards.push({ id, level, state: bestState(card, level) });
  draftChanged();
}

function draftChanged() {
  saveDraft();
  renderDraft();
  renderCoverageLine();
  renderScore();
  requestReport();
  previewSend();
}

function renderDraft() {
  $("draftName").value = state.draft.name;
  $("draftCards").innerHTML = state.draft.cards.map((c, i) => {
    const card = state.byId.get(c.id);
    if (!card) return `<div class="draft-row bad"><span class="name">#${c.id} (unknown)</span></div>`;
    const [power, damage] = card.evos[c.level] ?? [0, 0];
    const owned = card.owned[c.level]?.[c.state] ?? 0;
    return `<div class="draft-row${owned ? "" : " bad"}" data-index="${i}">` +
      `<div><span class="name">${esc(card.name)}</span>${solverBadge(card, c.level)} <span class="dim">${esc(card.clan)} · ${power}/${damage}` +
      `${c.state ? " · " + esc(c.state) : ""}${owned ? "" : " · not owned at this level"}</span></div>` +
      `<span class="stepper"><button class="icon" data-step="-1">−</button>L${c.level}<button class="icon" data-step="1">+</button></span>` +
      `<button class="icon" data-swap="1" title="Find a better owned card for this slot, scored against the opponent below">⇄</button>` +
      `<button class="icon" data-remove="1" title="Remove">×</button></div>`;
  }).join("") || '<div class="dim">Add cards from the collection, or load a saved deck.</div>';
}

let reportTimer = 0;
function requestReport() {
  clearTimeout(reportTimer);
  reportTimer = setTimeout(async () => {
    if (!state.draft.cards.length) {
      $("report").innerHTML = "";
      return;
    }
    try {
      const res = await fetch("/api/report", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ characters: state.draft.cards, night: night() }),
      });
      const report = await res.json();
      if (!res.ok) throw new Error(report.error ?? res.status);
      renderReport(report);
    } catch (e) {
      $("report").innerHTML = `<div class="note">No report: ${esc(e.message)}</div>`;
    }
  }, 150);
}

function diffWithSource() {
  const source = state.decks.find((d) => d.id === state.draft.sourceId);
  if (!source) return "";
  const key = (c) => `${c.id}`;
  const before = new Map(source.characters.map((c) => [key(c), c]));
  const after = new Map(state.draft.cards.map((c) => [key(c), c]));
  const name = (id) => esc(state.byId.get(id)?.name ?? `#${id}`);
  const parts = [];
  for (const [k, c] of after) {
    const old = before.get(k);
    if (!old) parts.push(`<span class="add">+ ${name(c.id)} L${c.level}</span>`);
    else if (old.level !== c.level) parts.push(`${name(c.id)} L${old.level}→L${c.level}`);
  }
  for (const [k, c] of before) if (!after.has(k)) parts.push(`<span class="del">− ${name(c.id)} L${c.level}</span>`);
  return `<div class="diff dim">Against saved "${esc(source.name)}": ${parts.length ? parts.join(", ") : "no change"}</div>`;
}

function renderReport(report) {
  const selected = Number($("format").value);
  const chips = report.formats.map((f) => {
    const cls = f.legal === true ? "ok" : f.legal === false ? "bad" : "unk";
    const mark = f.legal === true ? "✓" : f.legal === false ? "✗" : "?";
    const why = f.errors.map((e) => e.description).concat(f.unknown.map((u) => `not understood: ${u.name}`)).join("\n");
    return `<span class="${cls}${f.formatId === selected ? " sel" : ""}" title="${esc(why)}">${mark} ${esc(f.name)}</span>`;
  }).join("");
  const chosen = report.formats.find((f) => f.formatId === selected);
  const errors = chosen?.errors.length
    ? `<ul class="errs">${chosen.errors.map((e) => `<li>${esc(e.description)}</li>`).join("")}</ul>`
    : "";
  const clans = report.clans.map((c) => `${esc(c.clan)} ×${c.count}`).join(", ") +
    (report.leaders ? `, Leaders ×${report.leaders}` : "");
  const rows = report.cards.map((c, i) => {
    if (!c.known) return `<tr><td colspan="4" class="bad">#${c.id} L${c.level}: not in the captured card list</td></tr>`;
    const invalid = chosen?.invalidIndexes.includes(i) ? ' class="bad"' : "";
    const share = c.bonusLiveShare === undefined ? "—" : `${Math.round(c.bonusLiveShare * 100)}%`;
    return `<tr><td${invalid}>${esc(c.name)}<div class="dim">L${c.level}/${c.levelMax} · ${c.power}/${c.damage}</div></td>` +
      `<td>${esc(c.ability)}</td><td>${esc(c.bonus)}<div class="dim">live ${share}</div></td>` +
      `<td>${c.ownedExact ? `×${c.ownedExact}` : '<span class="bad">none</span>'}</td></tr>`;
  }).join("");
  const cap = chosen?.maxStars ? `/${chosen.maxStars}` : "";
  $("report").innerHTML = `<h3>${report.cards.length} cards · ${report.stars}${cap}★</h3>` +
    `<div class="chips">${chips}</div>${errors}<div class="dim">${clans}</div>${diffWithSource()}` +
    `<table><tr><th>Card</th><th>Ability</th><th>Bonus</th><th>Owned</th></tr>${rows}</table>` +
    report.notes.map((n) => `<div class="note">${esc(n)}</div>`).join("");
}

// ---- sending the draft to the site (the deck service's /api/apply) ------------------------
// Deck Lab cannot reach the site: the site's session is a cookie that only a page on
// urban-rivals.com carries. Send queues one request in the deck service; the userscript's
// UR Lab panel on Collection Pro shows it and saves it only when the owner clicks Apply
// there. This box shows a dry run of the same checks first, then the wait, then the outcome.
const send = { request: null, preview: null, timer: 0, previewTimer: 0 };
const COLLECTION_PRO = "https://www.urban-rivals.com/collection/pro/";
/** Queued and not over yet: waiting for the owner's click, or being saved after it. */
const waiting = (r) => r?.state === "pending" || r?.state === "applying";

function sendBody(extra = {}) {
  const target = $("sendTarget").value;
  return {
    characters: state.draft.cards,
    name: $("sendName").value,
    target: target === "new" ? "new" : { deckId: Number(target) },
    setCurrent: $("sendCurrent").checked,
    ...extra,
  };
}

function openSend() {
  $("sendTarget").innerHTML = '<option value="new">a new deck</option>' + state.decks.map((d) =>
    `<option value="${d.id}">over "${esc(d.name)}"${d.isCurrent ? " (current)" : ""} · ${d.characters.length} cards</option>`
  ).join("");
  // A request still waiting is shown as it was sent; otherwise a new deck, named after the draft.
  const r = waiting(send.request) ? send.request : null;
  $("sendTarget").value = r?.deckId && state.decks.some((d) => d.id === r.deckId) ? String(r.deckId) : "new";
  $("sendName").value = r ? r.name : state.draft.name.slice(0, 32);
  $("sendCurrent").checked = r ? r.setCurrent : false;
  $("sendBox").hidden = false;
  previewSend();
}

const summaryHtml = (s) =>
  `<div class="headline">${esc(s.headline)}</div>` +
  (s.lines.length ? `<ul>${s.lines.map((l) => `<li>${esc(l)}</li>`).join("")}</ul>` : "") +
  s.warnings.map((w) => `<div class="note">${esc(w)}</div>`).join("");

function outcomeHtml(r) {
  const when = r.finishedAt ? ` <span class="dim">(${esc(r.finishedAt.slice(11, 16))} UTC)</span>` : "";
  const what = `Last send, "${esc(r.name)}": `;
  if (r.state === "applied") {
    return `<div class="ok">${what}saved on the site and read back, it holds exactly the ${r.characters.length} cards sent. ` +
      `Reload Collection Pro to see it there.${when}</div>`;
  }
  if (r.state === "failed") {
    return `<div class="bad">${what}${r.result?.sent ? "the site was asked to save it, but " : "nothing was saved: "}` +
      `${esc(r.result?.error ?? "no reason given")}${when}</div>` +
      (r.result?.sent ? '<div class="dim">Check the deck in Collection Pro; data/deck_history.jsonl keeps its old cards.</div>' : "");
  }
  if (r.state === "dismissed") return `<div class="dim">${what}dismissed in the UR Lab panel, nothing was sent.${when}</div>`;
  if (r.state === "discarded") return `<div class="dim">${what}withdrawn, nothing was sent.${when}</div>`;
  return "";
}

function renderSend() {
  if ($("sendBox").hidden) return;
  const r = send.request;
  const busy = waiting(r);
  for (const id of ["sendTarget", "sendName", "sendCurrent"]) $(id).disabled = busy;
  // Once the panel has claimed it, it is being saved and can no longer be withdrawn.
  $("sendWithdraw").hidden = r?.state !== "pending";
  $("sendGo").hidden = busy;
  if (busy) {
    $("sendOut").innerHTML = summaryHtml(r.summary) + (r.state === "applying"
      ? '<div class="waiting">You clicked Apply in the UR Lab panel: it is saving the deck on the site now…</div>'
      : `<div class="waiting">Waiting for you to confirm in the UR Lab panel on <a href="${COLLECTION_PRO}" target="_blank" ` +
        'rel="noopener">Collection Pro</a> (it opens by itself within a few seconds). Nothing reaches the site until you ' +
        "click there.</div>");
    return;
  }
  const p = send.preview;
  const check = !p
    ? '<div class="dim">Checking…</div>'
    : p.errors
    ? `<div class="note">Cannot send it:</div><ul class="errs">${p.errors.map((e) => `<li>${esc(e)}</li>`).join("")}</ul>`
    : summaryHtml(p.summary);
  $("sendOut").innerHTML = (r ? `<div class="last">${outcomeHtml(r)}</div>` : "") + check;
  $("sendGo").disabled = !p || !!p.errors;
}

/** The same checks as a real send, without queueing anything. */
function previewSend() {
  clearTimeout(send.previewTimer);
  if ($("sendBox").hidden || waiting(send.request)) return renderSend();
  send.preview = null;
  renderSend();
  send.previewTimer = setTimeout(async () => {
    try {
      const res = await fetch("/api/apply", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(sendBody({ dryRun: true })),
      });
      const body = await res.json();
      send.preview = res.ok ? body : { errors: body.errors ?? [body.error ?? `the deck service answered ${res.status}`] };
    } catch (e) {
      send.preview = { errors: [`the deck service does not answer (${e.message})`] };
    }
    renderSend();
  }, 200);
}

async function queueSend(replace = false) {
  $("sendGo").disabled = true;
  try {
    const res = await fetch("/api/apply", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(sendBody(replace ? { replace: true } : {})),
    });
    const body = await res.json();
    if (res.status === 409 && body.pending) {
      const question = `${body.error}:\n\n${body.pending.summary.headline}\n\nReplace it with this one?`;
      if (confirm(question)) return queueSend(true);
      send.request = body.pending;
    } else if (!res.ok) {
      send.preview = { errors: body.errors ?? [body.error ?? `the deck service answered ${res.status}`] };
    } else send.request = body;
  } catch (e) {
    send.preview = { errors: [`the deck service does not answer (${e.message})`] };
  }
  renderSend();
  pollSend();
}

function pollSend() {
  clearTimeout(send.timer);
  if (!waiting(send.request)) return;
  send.timer = setTimeout(async () => {
    const was = send.request;
    try {
      const latest = await fetch("/api/apply").then((r) => r.json());
      send.request = latest.state === "none" ? null : latest;
      if (latest.id === was.id && latest.state === "applied") await savedOnSite(latest);
    } catch { /* the service is restarting; ask again */ }
    if (!waiting(send.request)) previewSend();
    else renderSend();
    pollSend();
  }, 1500);
}

/** The site now holds the draft: follow its deck, as the site's own loaddeck returned it. */
async function savedOnSite(request) {
  const deck = request.result?.deck;
  if (!deck) return;
  if (deck.isCurrent) for (const d of state.decks) d.isCurrent = false;
  state.decks = [...state.decks.filter((d) => d.id !== deck.id), deck];
  state.draft.name = deck.name;
  state.draft.sourceId = deck.id;
  saveDraft();
  try {
    state.history = await fetch("/api/apply/history").then((r) => r.json());
  } catch { /* the undo list refreshes on the next load */ }
  renderDeckChoices();
  renderOpponents();
  renderDraft();
  requestReport();
}

// ---- saved decks, deck-search results and the undo log, as drafts to load ----------------
function renderDeckChoices() {
  const decks = state.decks.map((d) =>
    `<option value="${d.id}">${esc(d.name)}${d.isCurrent ? " (current)" : ""} · ${d.characters.length}</option>`
  ).join("");
  const suggestions = state.suggestions.length
    ? `<optgroup label="Improved by deno task deck-search">${
      state.suggestions.map((s, i) => {
        const gain = s.check ? ` (${s.check.gain >= 0 ? "+" : "−"}${Math.abs(s.check.gain * 50).toFixed(1)} on unseen hands)` : "";
        const legal = s.keptLegal?.length ? `, legal in ${s.keptLegal.join(", ")}` : "";
        return `<option value="suggestion:${i}">${esc(s.start.name)} vs ${esc(s.format.name)}${s.night ? " night" : ""}: ` +
          `${s.swaps.length ? `${s.swaps.length} swaps${gain}` : "no better swap"}${esc(legal)}</option>`;
      }).join("")
    }</optgroup>`
    : "";
  // Every overwrite keeps the deck's old cards: loading them and sending them back undoes it.
  const undo = state.history.filter((h) => h.before).map((h, i) =>
    `<option value="history:${i}">"${esc(h.before.name)}" before ${esc(h.t.slice(0, 16).replace("T", " "))} UTC</option>`
  ).join("");
  $("loadDeck").innerHTML = '<option value="">Load a saved deck…</option>' + decks + suggestions +
    (undo ? `<optgroup label="As they were before Send to site">${undo}</optgroup>` : "");
}

// ---- wiring -----------------------------------------------------------------------------
async function main() {
  const [collection, decks, coverage, matchup, suggestions, apply, history] = await Promise.all([
    fetch("/api/collection").then((r) => r.json()),
    fetch("/api/decks").then((r) => r.json()),
    fetch("/api/coverage").then((r) => r.json()).catch(() => null),
    fetch("/api/matchup").then((r) => r.json()).catch(() => null),
    fetch("/api/suggestions").then((r) => r.json()).catch(() => []),
    fetch("/api/apply").then((r) => r.json()).catch(() => null),
    fetch("/api/apply/history").then((r) => r.json()).catch(() => []),
  ]);
  state.suggestions = Array.isArray(suggestions) ? suggestions : [];
  state.history = Array.isArray(history) ? history : [];
  send.request = apply?.id ? apply : null;
  state.coverage = coverage && !coverage.missing ? coverage : null;
  state.matchup = matchup;
  $("solverOnly").disabled = !state.coverage;
  state.cards = collection.cards;
  state.byId = new Map(state.cards.map((c) => [c.id, c]));
  state.formats = collection.formats;
  state.decks = decks.decks ?? [];
  state.legalByFormat = decks.legalByFormat ?? {};
  const at = collection.fetchedAt ?? {};
  $("freshness").textContent = `cards ${at.cards?.slice(0, 16) ?? "never"} · collection ${
    at.collection?.slice(0, 16) ?? "never"
  } · ${state.decks.length} decks`;

  $("format").innerHTML = state.formats.map((f) => `<option value="${f.id}">${esc(f.name)}</option>`).join("");
  $("format").value = String(state.formats.find((f) => f.name === "Tourney")?.id ?? state.formats[0]?.id ?? "");
  const clans = [...new Map(state.cards.map((c) => [c.clanId, c.clan])).entries()].sort((a, b) => a[1].localeCompare(b[1]));
  $("clan").innerHTML += clans.map(([id, name]) => `<option value="${id}">${esc(name)}</option>`).join("");
  renderDeckChoices();

  for (const id of ["search", "clan", "rarity", "levelMode", "sort", "ownedOnly", "legalOnly", "solverOnly"]) {
    $(id).addEventListener(id === "search" ? "input" : "change", () => {
      state.shown = PAGE;
      renderCollection();
    });
  }
  $("format").addEventListener("change", async () => {
    await loadMeta();
    renderOpponents();
    renderClans();
    renderCollection();
    requestReport();
  });
  $("night").addEventListener("change", () => {
    renderClans();
    renderCollection();
    renderDraft();
    renderCoverageLine();
    requestReport();
  });
  $("score").addEventListener("click", () => startScore());
  $("scoreOut").addEventListener("click", (e) => {
    const use = e.target.closest("[data-use]");
    const job = state.matchup;
    if (!use || job?.kind !== "swap") return;
    const pick = job.result.candidates[Number(use.dataset.use)];
    if (!pick || !state.draft.cards[job.slot]) return;
    state.draft.cards[job.slot] = { id: pick.card.id, level: pick.card.level, state: pick.card.state ?? "" };
    draftChanged();
  });
  $("clansToggle").addEventListener("click", () => {
    $("clans").hidden = !$("clans").hidden;
    renderClans();
  });
  $("clans").addEventListener("click", (e) => {
    const name = e.target.closest("[data-clan]")?.dataset.clan;
    const option = [...$("clan").options].find((o) => o.textContent === name);
    if (!option) return;
    $("clan").value = option.value;
    state.shown = PAGE;
    renderCollection();
  });
  $("cancel").addEventListener("click", async () => {
    try {
      state.matchup = await fetch("/api/matchup", { method: "DELETE" }).then((r) => r.json());
    } catch { /* the poll will catch up */ }
    renderScore();
    pollScore();
  });
  $("more").addEventListener("click", () => {
    state.shown += PAGE;
    renderCollection();
  });
  $("cards").addEventListener("click", (e) => {
    const button = e.target.closest("[data-add]");
    if (button) addToDraft(Number(button.dataset.add), Number(button.dataset.level));
  });
  $("draftCards").addEventListener("click", (e) => {
    const row = e.target.closest(".draft-row");
    if (!row) return;
    const entry = state.draft.cards[Number(row.dataset.index)];
    const card = state.byId.get(entry.id);
    if (e.target.dataset.swap) return startScore(Number(row.dataset.index));
    if (e.target.dataset.remove) state.draft.cards.splice(Number(row.dataset.index), 1);
    else if (e.target.dataset.step && card) {
      const level = entry.level + Number(e.target.dataset.step);
      if (!card.evos[level]) return;
      entry.level = level;
      entry.state = bestState(card, level);
    } else return;
    draftChanged();
  });
  $("draftName").addEventListener("input", () => {
    state.draft.name = $("draftName").value;
    saveDraft();
  });
  $("loadDeck").addEventListener("change", () => {
    const value = $("loadDeck").value;
    $("loadDeck").value = "";
    const undo = value.startsWith("history:") ? state.history.filter((h) => h.before)[Number(value.slice("history:".length))] : null;
    if (undo) {
      // Its source is the deck as it is now, so the diff line shows what sending it back undoes.
      state.draft = { name: undo.before.name, sourceId: undo.deckId, cards: undo.before.characters.map((c) => ({ ...c })) };
      draftChanged();
      return;
    }
    const suggestion = value.startsWith("suggestion:") ? state.suggestions[Number(value.slice("suggestion:".length))] : null;
    if (suggestion) {
      // Its source is the deck it started from, so the diff line shows the swaps.
      state.draft = {
        name: `${suggestion.start.name} (improved)`,
        sourceId: suggestion.start.id,
        cards: suggestion.characters.map((c) => ({ ...c })),
      };
      draftChanged();
      return;
    }
    const deck = state.decks.find((d) => d.id === Number(value));
    if (!deck) return;
    state.draft = { name: deck.name, sourceId: deck.id, cards: deck.characters.map((c) => ({ ...c })) };
    draftChanged();
  });
  $("clear").addEventListener("click", () => {
    state.draft = { name: "", sourceId: 0, cards: [] };
    draftChanged();
  });
  $("send").addEventListener("click", openSend);
  $("sendClose").addEventListener("click", () => {
    $("sendBox").hidden = true;
  });
  $("sendTarget").addEventListener("change", () => {
    const deck = state.decks.find((d) => d.id === Number($("sendTarget").value));
    if (deck && !$("sendName").value.trim()) $("sendName").value = deck.name;
    previewSend();
  });
  $("sendName").addEventListener("input", previewSend);
  $("sendCurrent").addEventListener("change", previewSend);
  $("sendGo").addEventListener("click", () => queueSend());
  $("sendWithdraw").addEventListener("click", async () => {
    try {
      const latest = await fetch("/api/apply", { method: "DELETE" }).then((r) => r.json());
      send.request = latest.state === "none" ? null : latest;
    } catch { /* the poll catches up */ }
    previewSend();
  });
  $("export").addEventListener("click", () => {
    const blob = new Blob([JSON.stringify(state.draft, null, 1)], { type: "application/json" });
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = `${state.draft.name || "draft"}.json`;
    a.click();
    URL.revokeObjectURL(a.href);
  });

  loadDraft();
  await loadMeta();
  renderOpponents();
  renderCollection();
  renderDraft();
  renderCoverageLine();
  renderScore();
  pollScore();
  requestReport();
  // A request still waiting in the UR Lab panel is shown, and followed, after a reload too.
  if (waiting(send.request)) {
    openSend();
    pollSend();
  }
}

main().catch((e) => {
  document.body.insertAdjacentHTML("beforeend", `<p class="note">Deck Lab could not load: ${esc(e.message)}</p>`);
});
