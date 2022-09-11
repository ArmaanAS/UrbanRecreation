import UROAuth from 'urban-rivals-oauth';
import { config } from 'dotenv';
import process from 'process';
import { writeFileSync, readFileSync } from "fs";

config();

const urApi = new UROAuth({
  key: process.env.API_KEY,
  secret: process.env.API_SECRET,
});


export default async function callAPI(method, params) {
  const { items, context } = await urApi.query(method, params);

  return { items, context };
}

let accessToken;
try {
  accessToken = JSON.parse(readFileSync("./tokens.json").toString());
  console.log("accessToken", accessToken);
  urApi.accessToken = accessToken;

  // Save accessTokens to `tokens.json`
  writeFileSync("./tokens.json", JSON.stringify(accessToken));
} catch (e) {
  console.log("\nFailed to read ./tokens.json\n");
}


// Get initial request token 
const requestToken = await urApi.getRequestToken();
console.log('getRequestToken', requestToken);



// Get player's accessToken, to make requests with
if (accessToken) {
  // Test accessTokens work still
  try {
    const result = await callAPI("characters.getCharacters", {
      charactersIDs: 123,
    });
  } catch (e) {
    console.log("\naccessToken has expired\n");
    accessToken = undefined;
  }
}

// Wait for user to validate accessToken with link
if (!accessToken) {
  // Log URL to authorize your account
  const url = urApi.getAuthorizeUrl('about:new');
  console.log(url);

  // Wait for enter input
  console.info("\nPress <ENTER> when ready\n");
  process.stdin.resume();
  await new Promise(res => process.stdin.once("data", res));
  process.stdin.end();

  await urApi.getAccessToken();

  // Save accessTokens to `tokens.json`
  writeFileSync("./tokens.json", JSON.stringify(urApi.accessToken));
}
