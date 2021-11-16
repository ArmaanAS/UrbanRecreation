import callAPI from "./UR_API.js";
import fs from 'fs';
import 'colors';

console.info("Requesting all character data from API...".yellow);
console.time('Request');
const { items } = await callAPI('characters.getCharacters', {
  maxLevels: true
});
console.timeEnd('Request');

console.info("Writing data to './data.json'".yellow);
console.time('Write')
fs.writeFileSync('./data.json', JSON.stringify(items));
console.timeEnd('Write')

console.info("Finished writing to './data.json'".green);