import callAPI from "./UR_API.ts";
import "colors";

console.info("Requesting all character data from API...".yellow);
console.time("Request");
const { items } = await callAPI("characters.getCharacters", {
  maxLevels: true,
});
console.timeEnd("Request");

console.info("Writing data to './cards.json'".yellow);
await Deno.writeTextFile("./data/cards.json", JSON.stringify(items));
console.info("Finished writing to './cards.json'".green);

const file = "./CompileAbilities.js";
await import(file);
