import "colors";
import { assertEquals } from "@std/assert";
import Canvas from "@/utils/Canvas.ts";

Deno.test("clipped styled canvas text closes its colour and background", () => {
  const canvas = new Canvas(5, 1);
  canvas.write(0, "a deliberately long label".blue.bgWhite);

  assertEquals(canvas.lines[0].replace(/\x1b\[[0-9;]*m/g, "").length, 5);
  assertEquals(canvas.lines[0].endsWith("\x1b[0m"), true);
});
