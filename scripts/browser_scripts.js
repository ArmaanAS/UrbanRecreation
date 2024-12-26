async function findString(searchStr) {
  const heap = unityGame.Module.HEAPU16;

  // Convert rest of string to UTF-16 LE bytes for comparison
  const bytes = [...searchStr].map(c => c.charCodeAt(0));

  let pos = -1;
  outer: while (true) {
    pos = heap.indexOf(bytes[0], pos + 1);

    if (pos === -1) break;

    // Check if rest of string matches
    for (let j = 1; j < bytes.length; j++) {
      if (heap[pos + j] !== bytes[j]) {
        continue outer;
      }
    }

    const slice = heap.slice(pos - 2, pos + bytes.length + 2);
    console.log(pos, slice);
    console.log(String.fromCharCode(...slice));
  }
  console.log("Done");
}

async function findStringsOfLength(len, log = true) {
  const heap = unityGame.Module.HEAPU16;

  const strings = [];
  const unique = new Set();

  let pos = -1;
  outer: while (true) {
    pos = heap.indexOf(len, pos + 1);

    if (pos === -1) break;
    if (heap[pos + 1] !== 0) continue;
    if (heap[pos + 2 + len] !== 0) continue;
    if (heap[pos + 2 + len + 1] !== 0) continue;

    for (let j = 0; j < len; j++) {
      const char = heap[pos + 2 + j];
      if (char < 32 || char > 126) {
        continue outer;
      }
    }

    const string = String.fromCharCode(...heap.slice(pos + 2, pos + 2 + len));
    if (unique.has(string)) continue;
    if (/^\d+$/.test(string)) continue;
    if (/^[0-9a-fA-F]+$/.test(string)) continue;
    strings.push({ pos, string });
    unique.add(string);
  }
  if (log) console.table(strings);
  else console.log("Done", len);
  return strings;
}

let allStrings = [];
for (let i = 3; i < 20; i++) {
  const strings = await findStringsOfLength(i, false);
  allStrings.push(...strings);
}
console.table(allStrings);

async function findJsonObjects() {
  const heap = unityGame.Module.HEAPU16;

  const openBracketChar = "{".charCodeAt(0);
  const closeBracketChar = "}".charCodeAt(0);

  const objects = [];

  let pos = -1;
  while (true) {
    pos = heap.indexOf(openBracketChar, pos + 1);

    if (pos === -1) break;
    // if (heap[pos - 1] !== 0) continue;
    // if (heap[pos - 2] < 32) continue;

    const len = heap[pos - 2] | (heap[pos - 1] << 16);

    if (len < 32 || len > 131_072) continue;
    if (heap[pos + len - 1] !== closeBracketChar) continue;

    const string = String.fromCharCode(...heap.slice(pos, pos + len));
    try {
      const data = JSON.parse(string);
      objects.push({ pos, len, string, data });
    } catch (_) { }
  }
  console.table(objects);
  return objects;
}

findJsonObjects();

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
  await fetch("http://localhost:8000", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "Access-Control-Allow-Origin": "*",
    },
    body: JSON.stringify({
      h1: p0 === 19309601 ? h0 : h1,
      h2: p0 === 19309601 ? h1 : h0,
      life,
      pillz,
      first,
    }),
  });
}
