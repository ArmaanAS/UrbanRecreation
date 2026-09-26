// The deck service answers only Deck Lab's own page, the site (the userscript panel) and
// local tools, and it never needs the owner's data to refuse a bad request.
import { assertEquals, assertStringIncludes } from "@std/assert";
import { handle } from "@/decks/Service.ts";

const req = (path: string, init: RequestInit & { origin?: string } = {}) => {
  const headers = new Headers(init.headers);
  if (init.origin) headers.set("origin", init.origin);
  return new Request(`http://127.0.0.1:8788${path}`, { ...init, headers });
};

Deno.test("Deck Lab's page and script are served", async () => {
  const page = await handle(req("/"));
  assertEquals(page.status, 200);
  assertStringIncludes(await page.text(), "<title>Deck Lab</title>");
  const script = await handle(req("/app.js"));
  assertEquals(script.headers.get("content-type"), "text/javascript; charset=utf-8");
  await script.body?.cancel();
});

Deno.test("only the site, Deck Lab itself and local tools may call it", async () => {
  assertEquals((await handle(req("/api/formats", { origin: "https://evil.example" }))).status, 403);
  for (const origin of ["https://www.urban-rivals.com", "http://127.0.0.1:8788", "http://localhost:8788"]) {
    const res = await handle(req("/api/formats", { origin }));
    assertEquals(res.status, 200, origin);
    await res.body?.cancel();
  }
  const preflight = await handle(req("/api/report", { method: "OPTIONS", origin: "https://www.urban-rivals.com" }));
  assertEquals(preflight.status, 204);
  assertEquals(preflight.headers.get("access-control-allow-origin"), "https://www.urban-rivals.com");
});

Deno.test("a malformed deck is refused before any data is read", async () => {
  const bad = await handle(req("/api/report", { method: "POST", body: JSON.stringify({ characters: [{ id: 1, level: 9 }] }) }));
  assertEquals(bad.status, 400);
  const notJson = await handle(req("/api/report", { method: "POST", body: "{" }));
  assertEquals(notJson.status, 400);
  await bad.body?.cancel();
  await notJson.body?.cancel();
});
