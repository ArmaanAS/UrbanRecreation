import Player from '../../src/ts/Player';

test("Player", () => {
  const p = new Player(12, 12, 0)

  expect(p.name).toBe('Player');
  expect(p.life).toBe(12);
  expect(p.pillz).toBe(12);
  expect(p.won).toBe(undefined);
  expect(p.wonPrevious).toBe(undefined);

  p.life = 24;
  expect(p.life).toBe(24);

  p.pillz = 14
  expect(p.life).toBe(24);
  expect(p.pillz).toBe(14);

  p.won = true;
  expect(p.life).toBe(24);
  expect(p.pillz).toBe(14);
  expect(p.won).toBe(true);

  p.wonPrevious = false;
  expect(p.life).toBe(24);
  expect(p.pillz).toBe(14);
  expect(p.won).toBe(true);
  expect(p.wonPrevious).toBe(false);

  p.life = -1;
  expect(p.life).toBe(0);

  // p.pillz = -13248974;
  // expect(p.pillz).toBe(0);
})