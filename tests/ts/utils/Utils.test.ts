import { shiftRange, alternateRange } from '../../../src/ts/utils/Utils'

test("alternateRange generator", () => {
  expect([...alternateRange(12)])
    .toEqual([0, 12, 1, 11, 2, 10, 3, 9, 4, 8, 5, 7, 6]);

  expect([...alternateRange(9)])
    .toEqual([0, 9, 1, 8, 2, 7, 3, 6, 4, 5]);

  expect([...alternateRange(3)])
    .toEqual([0, 3, 1, 2]);

  expect([...alternateRange(2)])
    .toEqual([0, 2, 1]);

  expect([...alternateRange(1)])
    .toEqual([0, 1]);

  expect([...alternateRange(15)])
    .toEqual([0, 15, 1, 14, 2, 13, 3, 12, 4, 11, 5, 10, 6, 9, 7, 8]);
})

test("shiftRange generator", () => {
  expect([...shiftRange(12)])
    .toEqual([12, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 11]);

  expect([...shiftRange(9)])
    .toEqual([9, 6, 0, 1, 2, 3, 4, 5, 7, 8]);

  expect([...shiftRange(3)])
    .toEqual([3, 0, 1, 2]);

  expect([...shiftRange(2)])
    .toEqual([2, 0, 1]);

  expect([...shiftRange(1)])
    .toEqual([1, 0]);

  expect([...shiftRange(15)])
    .toEqual([15, 12, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 14]);
})