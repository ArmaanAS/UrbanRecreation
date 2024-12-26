import { assertEquals } from "@std/assert";

class A {
  a = 1;
}

Deno.test("Worker", async () => {
  const url = new URL("worker.ts", import.meta.url);
  const worker = new Worker(url, { type: "module" });

  worker.postMessage(new A());
  console.log(new A());
  const data = await new Promise((r) => worker.onmessage = (e) => r(e.data));

  assertEquals(data, "Hello from worker!");

  worker.terminate();
});
