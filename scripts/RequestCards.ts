import callAPI from "./UR_API.ts";
import "colors";

console.info("Requesting all character data from API...".yellow);
console.time("Request");
const { items } = await callAPI("characters.getCharacters", {
  maxLevels: true,
});
console.timeEnd("Request");

console.info("Writing data to './data.json'".yellow);
console.time("Write");
Deno.writeTextFileSync("./data/data.json", JSON.stringify(items));
console.timeEnd("Write");

console.info("Finished writing to './data.json'".green);

const file = "./CompileAbilities.js";
await import(file);
