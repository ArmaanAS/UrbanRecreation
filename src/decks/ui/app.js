// Deck Lab: browse the collection, draft a deck and see its report, all from the data the
// log server captured while Collection Pro was open. Served by src/decks/Service.ts. It is
// read-only: nothing here talks to urban-rivals.com, so it cannot change the account.
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

async function startScore() {
  const choice = $("opponent").value;
  if (!choice) return;
  const opponent = choice === "field" ? { format: format()?.id } : { deck: Number(choice.slice("deck:".length)) };
  try {
    const res = await fetch("/api/matchup", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ characters: state.draft.cards, night: night(), n: Number($("hands").value), opponent }),
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
  if (running) {
    out.innerHTML = `<div>Scoring against ${esc(job.against)}${when}…</div>` + (job.total
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
  const changed = JSON.stringify(job.deck) !== JSON.stringify(state.draft.cards.map((c) => [c.id, c.level]))
    ? '<div class="dim">The draft has changed since this score.</div>'
    : "";
  const margin = r.stderr == null ? "" : ` <span class="dim">± ${(r.stderr * 50).toFixed(1)}</span>`;
  const head = r.scored ? `<div class="score">${pct(r.percent)}${margin}</div>` : '<div class="score bad">Nothing could be scored</div>';
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
    worst + refusals +
    '<div class="dim small">Each hand pair is solved exactly with both first movers, playing the advisor\'s conservative ' +
    "policy, which never relies on guessing hidden pillz. 50% is even. The opposing hands are the same every time, so " +
    "use it to compare drafts; it is not a win rate.</div>";
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

// ---- wiring -----------------------------------------------------------------------------
async function main() {
  const [collection, decks, coverage, matchup] = await Promise.all([
    fetch("/api/collection").then((r) => r.json()),
    fetch("/api/decks").then((r) => r.json()),
    fetch("/api/coverage").then((r) => r.json()).catch(() => null),
    fetch("/api/matchup").then((r) => r.json()).catch(() => null),
  ]);
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
  $("loadDeck").innerHTML += state.decks.map((d) =>
    `<option value="${d.id}">${esc(d.name)}${d.isCurrent ? " (current)" : ""} · ${d.characters.length}</option>`
  ).join("");

  for (const id of ["search", "clan", "rarity", "levelMode", "sort", "ownedOnly", "legalOnly", "solverOnly"]) {
    $(id).addEventListener(id === "search" ? "input" : "change", () => {
      state.shown = PAGE;
      renderCollection();
    });
  }
  $("format").addEventListener("change", async () => {
    await loadMeta();
    renderOpponents();
    renderCollection();
    requestReport();
  });
  $("night").addEventListener("change", () => {
    renderCollection();
    renderDraft();
    renderCoverageLine();
    requestReport();
  });
  $("score").addEventListener("click", startScore);
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
    const deck = state.decks.find((d) => d.id === Number($("loadDeck").value));
    $("loadDeck").value = "";
    if (!deck) return;
    state.draft = { name: deck.name, sourceId: deck.id, cards: deck.characters.map((c) => ({ ...c })) };
    draftChanged();
  });
  $("clear").addEventListener("click", () => {
    state.draft = { name: "", sourceId: 0, cards: [] };
    draftChanged();
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
}

main().catch((e) => {
  document.body.insertAdjacentHTML("beforeend", `<p class="note">Deck Lab could not load: ${esc(e.message)}</p>`);
});
