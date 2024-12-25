import UROAuth from "urban-rivals-oauth";
import accessToken from "../tokens.json" with { type: "json" };

const urApi = new UROAuth({
  key: Deno.env.get("API_KEY"),
  secret: Deno.env.get("API_SECRET"),
});

urApi.accessToken = accessToken;

export default async function callAPI(method: string, params: object) {
  const { items, context } = await urApi.query(method, params);

  return { items, context };
}

// Get initial request token
const requestToken: string = await urApi.getRequestToken();
console.log("getRequestToken", requestToken);

// Test accessTokens work still
let isTokenValid = true;
try {
  await callAPI("characters.getCharacters", {
    charactersIDs: 123,
  });
} catch (_) {
  console.log("\naccessToken has expired\n");
  isTokenValid = false;
}

// Wait for user to validate accessToken with link
if (!isTokenValid) {
  // Log URL to authorize your account
  const url = urApi.getAuthorizeUrl("about:new");
  console.log(url);

  // Wait for enter input
  alert("Press <ENTER> when ready");

  await urApi.getAccessToken();

  // Save accessTokens to `tokens.json`
  Deno.writeTextFileSync("./tokens.json", JSON.stringify(urApi.accessToken));
}
