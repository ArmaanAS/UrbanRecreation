async function findBattleStatus() {
  const heap = unityGame.Module.HEAPU16;

  const start = "{\"battles.status\":";

  // Convert rest of string to UTF-16 LE bytes for comparison
  const bytes = [...start].map(c => c.charCodeAt(0));
  const closeBracketChar = "}".charCodeAt(0);

  const objects = [];

  let pos = -1;
  outer: while (true) {
    pos = heap.indexOf(bytes[0], pos + 1);

    if (pos === -1) break;

    const len = heap[pos - 2] | (heap[pos - 1] << 16);

    if (len > 131_072) continue;
    if (heap[pos + len - 1] !== closeBracketChar) continue;

    // Check if rest of string matches
    for (let j = 1; j < bytes.length; j++) {
      if (heap[pos + j] !== bytes[j]) {
        continue outer;
      }
    }

    const string = String.fromCharCode(...heap.slice(pos, pos + len));
    try {
      const object = JSON.parse(string);
      const data = object["battles.status"].data.Battle;
      objects.push({ pos, len, data });
    } catch (_) { }
  }
  console.log("Done", objects);

  let highestCreationTime = 0;
  let highestRound = 0;
  let highest = null;
  for (const { data } of objects) {
    if (data.creationTime > highestCreationTime || (data.creationTime === highestCreationTime && data.round > highestRound)) {
      highestCreationTime = data.creationTime;
      highestRound = data.round;
      highest = data;
    }
  }

  return highest;
}

{
  document.getElementById("unity-container").style.width = "100%";
  const data = await findBattleStatus();
  const p0 = data.Player0.Player.id;
  const p1 = data.Player1.Player.id;
  const h0 = data.Player0.Characters.map(c => c.id);
  const h1 = data.Player1.Characters.map(c => c.id);
  const first = data.turnPlayerID === 19309601;
  const life = data.Player0.baseLife;
  const pillz = data.Player0.basePillz;
  await fetch("http://localhost:8080", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      cards: p0 === 19309601 ? [...h0, ...h1] : [...h1, ...h0],
      life,
      pillz,
      flip: first ? 0 : 1,
    }),
  });
}