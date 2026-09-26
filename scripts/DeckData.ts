// Rebuild the deck builder's data files from a raw capture log.
//
//   deno task deck-data                          # reads ur_log.jsonl
//   deno task deck-data older.jsonl ur_log.jsonl # several logs, oldest first
//
// The log server keeps the same files up to date as Collection Pro loads (see
// scripts/DeckCapture.ts); this is for seeding them from traffic captured before that, or
// after deleting them. Records are applied in log order, so the newest copy of everything
// wins, and a card list assembled from several visits keeps each card's latest row.
import "colors";
import { TextLineStream } from "@std/streams/text-line-stream";
import { absorbRecord, newDeckStore, saveDeckStore } from "./DeckCapture.ts";

const paths = Deno.args.length ? Deno.args : ["ur_log.jsonl"];
const store = newDeckStore();
let lines = 0, used = 0;

for (const path of paths) {
  const file = await Deno.open(path);
  const records = file.readable.pipeThrough(new TextDecoderStream()).pipeThrough(new TextLineStream());
  for await (const line of records) {
    lines++;
    // Cheap filter first: the log is mostly battle polls and WebSocket frames.
    if (!line.includes("/ajax/collection") && !line.includes("collections.decks")) continue;
    try {
      if (absorbRecord(JSON.parse(line), store)) used++;
    } catch {
      // A truncated last line or a pre-JSON record; nothing to take from it.
    }
  }
}

const written = await saveDeckStore(store);
console.log(`${lines} records read from ${paths.join(", ")}, ${used} collection or deck records used`.green);
console.log(
  `${store.formats?.formats.length ?? 0} formats, ${store.cards.size} cards, ` +
    `${store.owned.size} owned, ${store.decks.size} decks`,
);
for (const f of written) console.log(`wrote ${f}`);
if (!store.formats) console.log("no deckformatsdata in the log: open Collection Pro with the log server running".yellow);
