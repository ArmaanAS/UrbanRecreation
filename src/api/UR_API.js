import UROAuth from 'urban-rivals-oauth'
import { config } from 'dotenv'
import process from 'process'

config();

const urApi = new UROAuth({
  key: process.env.API_KEY,
  secret: process.env.API_SECRET,
});


export default async function callAPI(method, params) {
  const { items, context } = await urApi.query(method, params)

  return { items, context };
}

// Get initial request token 
const requestToken = await urApi.getRequestToken();
console.log('getRequestToken', requestToken);

// Log URL to authorize your account
const url = urApi.getAuthorizeUrl('https://localhost/');
console.log(url);



// Wait for enter input
console.info("\nPress <ENTER> when ready\n");
process.stdin.resume();
await new Promise(res => process.stdin.once("data", res));
process.stdin.end();

// Get players accessToken, to make requests with
await urApi.getAccessToken();

const result = await callAPI("characters.getCharacters", {
  charactersIDs: 123,
});
console.debug(result);