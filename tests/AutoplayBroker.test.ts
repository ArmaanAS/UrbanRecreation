// The autoplay broker's rules: what a local driver may ask the game tab to send, and the
// long-poll hand-over between them (scripts/AutoplayBroker.ts).
import { assert, assertEquals } from "@std/assert";
import { AutoplayBroker } from "../scripts/AutoplayBroker.ts";

/** Run `request` against a fake game tab that answers each command with `answer(cmd)`. */
async function roundTrip(broker: AutoplayBroker, body: unknown, answer: (cmd: Record<string, unknown>) => unknown) {
  const pending = broker.request(body, 1000);
  const cmd = await broker.next(1000);
  if (cmd) broker.result({ id: cmd.id, ok: true, result: answer(cmd as unknown as Record<string, unknown>) });
  return await pending;
}

const joined = { data: { room: { id: 6, name: "Training" } } };

Deno.test("only listed calls and deck actions reach the game tab", async () => {
  const broker = new AutoplayBroker();
  for (const body of [
    { call: "market.purchase", params: {} },
    { call: "collections.evolve" },
    { deck: "deletedeck", fields: [] },
    { deck: "savedeck", fields: [["id", 1]] },
    { call: "rooms.join", params: { id: 16193 } },
    "battles.play",
  ]) {
    const out = await broker.request(body, 50);
    assert(!out.ok, JSON.stringify(body));
  }
  assertEquals(broker.state().queued, 0);
});

Deno.test("quickBattle needs a successful join to Training right before it", async () => {
  const broker = new AutoplayBroker();
  assert(!(await broker.request({ call: "battles.quickBattle" }, 50)).ok);
  const join = await roundTrip(broker, { call: "rooms.join", params: { id: 6 } }, () => joined);
  assert(join.ok);
  const quick = await roundTrip(broker, { call: "battles.quickBattle" }, () => ({ data: null }));
  assert(quick.ok);
  // The join window closes with the quickBattle itself...
  assert(!(await broker.request({ call: "battles.quickBattle" }, 50)).ok);
  // ...and with any other call in between.
  await roundTrip(broker, { call: "rooms.join", params: { id: 6 } }, () => joined);
  await roundTrip(broker, { call: "general.refreshPlayer" }, () => ({ data: {} }));
  assert(!(await broker.request({ call: "battles.quickBattle" }, 50)).ok);
  // A join the site refused does not open it.
  await roundTrip(broker, { call: "rooms.join", params: { id: 6 } }, () => ({ errors: [{ message: "no" }] }));
  assert(!(await broker.request({ call: "battles.quickBattle" }, 50)).ok);
});

Deno.test("play and quit only on a battle seen with the Training rule", async () => {
  const broker = new AutoplayBroker();
  const play = { call: "battles.play", params: { id: 77, characterInBattleID: 1, pillz: 0, fury: false } };
  assert(!(await broker.request(play, 50)).ok);
  await roundTrip(broker, { call: "battles.status", params: { id: 88 } }, () => ({ data: { battle: { id: 88, battleRuleId: 10 } } }));
  assert(!(await broker.request({ call: "battles.quit", params: { id: 88 } }, 50)).ok);
  await roundTrip(broker, { call: "battles.status", params: { id: 77 } }, () => ({ data: { battle: { id: 77, battleRuleId: 2 } } }));
  const out = await roundTrip(broker, play, () => ({ data: null }));
  assert(out.ok);
});

Deno.test("a command waits for the next poll, and an unanswered one times out", async () => {
  const broker = new AutoplayBroker();
  const poll = broker.next(1000);
  const pending = broker.request({ call: "battles.ongoingBattleID" }, 1000);
  const cmd = await poll;
  assertEquals((cmd as { call: string }).call, "battles.ongoingBattleID");
  assert(broker.result({ id: cmd!.id, ok: false, error: "tab closed" }));
  assertEquals(await pending, { ok: false, error: "tab closed" });
  assert(!broker.result({ id: cmd!.id, ok: true }), "an answer counts once");

  const lost = await broker.request({ call: "battles.ongoingBattleID" }, 30);
  assert(!lost.ok && /no answer/.test(lost.error));
  assertEquals(broker.state().queued, 0, "a timed-out command is not handed out later");
  assertEquals(await broker.next(10), null);
});

Deno.test("a second poll takes over from the first", async () => {
  const broker = new AutoplayBroker();
  const first = broker.next(1000);
  const second = broker.next(1000);
  assertEquals(await first, null);
  const pending = broker.request({ call: "general.refreshPlayer" }, 1000);
  const cmd = await second;
  broker.result({ id: cmd!.id, ok: true, result: { data: {} } });
  assert((await pending).ok);
});
