// Shrink a raw userscript log (ur_log.jsonl) by dropping response bodies nothing can use.
//
//   deno task prune                        # report what would go from ur_log.jsonl
//   deno task prune --replace              # ...and rewrite the file in place
//   deno task prune other.jsonl --replace
//
// The capture pipeline only ever reads records whose payload.u contains /api/private/v2/
// (see parseApi in BattleCapture.ts). Those pass through byte for byte, and a second pass
// over the output checks that every one of them survived unchanged; nothing is replaced
// unless that check passes. Other records keep their request line (method, URL, status)
// but lose a response body that is binary or oversized.
//
// Why this is needed: the userscript reads every response with res.text(), so the WebGL
// asset bundles the game streams in (UnityFS, several MB each) arrive as lossy UTF-8 —
// ~34% U+FFFD replacement characters, i.e. bytes that are already destroyed and cannot be
// recovered from the log even in principle. They were 65% of the first real log: 3.5 GB
// of 5.4 GB, for 289 lines out of 26227. ur-logger.user.js >= 0.5 and log_server.ts now
// elide these on the way in, so this script is for logs captured before that.
import "colors";
import { TextLineStream } from "@std/streams/text-line-stream";

const MAX_RESP = 256 * 1024; // keep any non-API body smaller than this, whatever it is
const FAST_PATH = 8 * 1024; // lines below this can never need pruning: don't even parse

const isApi = (line: string) => line.includes("/api/private/v2/");
const label = (s: string) => `[dropped by PruneLog: ${s.length} chars${s.includes("\uFFFD") ? " of binary" : ""}]`;
const oversized = (v: unknown): v is string => typeof v === "string" && (v.length > MAX_RESP || v.includes("\uFFFD"));

/** FNV-1a over the API lines, so the verify pass can prove they came through untouched. */
function fnv(hash: bigint, s: string): bigint {
  for (let i = 0; i < s.length; i++) {
    hash = BigInt.asUintN(64, (hash ^ BigInt(s.charCodeAt(i))) * 1099511628211n);
  }
  return hash;
}

/** Stream `path` from `start` to `end`, yielding one line at a time. */
async function* lines(path: string, start = 0, end?: number) {
  const f = await Deno.open(path, { read: true });
  if (start) await f.seek(start, Deno.SeekMode.Start);
  let left = end === undefined ? Infinity : end - start;
  const cut = new TransformStream<Uint8Array, Uint8Array>({
    transform(chunk, c) {
      if (left <= 0) return;
      c.enqueue(chunk.length > left ? chunk.subarray(0, left) : chunk);
      left -= chunk.length;
    },
  });
  yield* f.readable.pipeThrough(cut).pipeThrough(new TextDecoderStream()).pipeThrough(new TextLineStream());
}

interface Stats { lines: number; api: number; hash: bigint; pruned: number }

/** Read [start, end) of `src`, write the pruned form to `out`, and tally what happened. */
async function prune(src: string, out: Deno.FsFile, start: number, end: number, st: Stats) {
  const enc = new TextEncoder();
  let buf = "";
  const flush = async (force: boolean) => {
    if (buf.length > (force ? 0 : 1 << 20)) {
      await out.write(enc.encode(buf));
      buf = "";
    }
  };
  for await (const line of lines(src, start, end)) {
    st.lines++;
    let keep = line;
    if (line.length > FAST_PATH && !isApi(line)) {
      try {
        const rec = JSON.parse(line);
        const p = rec.payload;
        let hit = false;
        if (p && typeof p === "object") {
          for (const k of ["resp", "body"]) {
            if (oversized(p[k])) {
              p[k] = label(p[k]);
              hit = true;
            }
          }
        }
        if (hit) {
          keep = JSON.stringify(rec);
          st.pruned++;
        }
      } catch { /* not JSON: the server logs it verbatim too, so keep it verbatim */ }
    }
    if (isApi(line)) {
      st.api++;
      st.hash = fnv(st.hash, line);
    }
    buf += keep + "\n";
    await flush(false);
  }
  await flush(true);
}

/** Re-read the written file and recompute the API-record count and hash. */
async function verify(path: string) {
  let api = 0, hash = 14695981039346656037n;
  for await (const line of lines(path)) {
    if (isApi(line)) {
      api++;
      hash = fnv(hash, line);
    }
  }
  return { api, hash };
}

// ---------------------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------------------
const args = [...Deno.args];
const replace = args.includes("--replace");
if (replace) args.splice(args.indexOf("--replace"), 1);
const SRC = args[0] ?? "ur_log.jsonl";
const OUT = SRC + ".pruned";

const mb = (b: number) => (b / 1024 / 1024).toFixed(1) + " MB";
const st: Stats = { lines: 0, api: 0, hash: 14695981039346656037n, pruned: 0 };
const out = await Deno.open(OUT, { write: true, create: true, truncate: true });

// The log server appends while we work, so prune up to the size we saw at the start and
// then catch up on whatever arrived, until the file stops growing.
let done = 0;
for (let pass = 0; pass < 10; pass++) {
  const size = (await Deno.stat(SRC)).size;
  if (size <= done) break;
  await prune(SRC, out, done, size, st);
  if (pass === 0) console.log(`${SRC}: ${mb(size)}, ${st.lines} lines`.cyan);
  else console.log(`  caught up on ${mb(size - done)} appended while pruning`.gray);
  done = size;
}
out.close();

const sizeIn = (await Deno.stat(SRC)).size, sizeOut = (await Deno.stat(OUT)).size;
const v = await verify(OUT);
const ok = v.api === st.api && v.hash === st.hash;
console.log(
  `pruned ${st.pruned} response bodies\n` +
    `  ${mb(sizeIn)} → ${mb(sizeOut)}  (${(100 - sizeOut / sizeIn * 100).toFixed(1)}% smaller)
` +
    `  ${st.api} /api/private/v2/ records ` + (ok ? "verified identical in the output".green : "MISMATCH IN OUTPUT".red),
);
if (!ok) {
  console.error(`refusing to replace ${SRC}; the pruned file is at ${OUT}`.red);
  Deno.exit(1);
}
if (!replace) {
  console.log(`wrote ${OUT} — re-run with --replace to swap it in`.yellow);
} else {
  await Deno.rename(OUT, SRC);
  console.log(`replaced ${SRC}`.green);
}
