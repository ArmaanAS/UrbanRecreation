import { alternateRange, shiftRange } from "@/utils/Utils.ts";
import { assertEquals } from "@std/assert";

Deno.test("alternateRange generator", () => {
  assertEquals(
    [...alternateRange(12)],
    [0, 12, 1, 11, 2, 10, 3, 9, 4, 8, 5, 7, 6],
  );
  assertEquals(
    [...alternateRange(9)],
    [0, 9, 1, 8, 2, 7, 3, 6, 4, 5],
  );
  assertEquals(
    [...alternateRange(3)],
    [0, 3, 1, 2],
  );
  assertEquals(
    [...alternateRange(2)],
    [0, 2, 1],
  );
  assertEquals(
    [...alternateRange(1)],
    [0, 1],
  );
  assertEquals(
    [...alternateRange(15)],
    [0, 15, 1, 14, 2, 13, 3, 12, 4, 11, 5, 10, 6, 9, 7, 8],
  );
});

Deno.test("shiftRange generator", () => {
  assertEquals(
    [...shiftRange(12)],
    [12, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 11],
  );
  assertEquals(
    [...shiftRange(9)],
    [9, 6, 0, 1, 2, 3, 4, 5, 7, 8],
  );
  assertEquals(
    [...shiftRange(3)],
    [3, 0, 1, 2],
  );
  assertEquals(
    [...shiftRange(2)],
    [2, 0, 1],
  );
  assertEquals(
    [...shiftRange(1)],
    [1, 0],
  );
  assertEquals(
    [...shiftRange(15)],
    [15, 12, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 14],
  );
});
