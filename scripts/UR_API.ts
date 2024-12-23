import UROAuth from "urban-rivals-oauth";

const urApi = new UROAuth({
  key: Deno.env.get("API_KEY"),
  secret: Deno.env.get("API_SECRET"),
});

export default async function callAPI(method: string, params: object) {
  const { items, context } = await urApi.query(method, params);

  return { items, context };
}

let accessToken: string | undefined;
try {
  accessToken = JSON.parse(Deno.readFileSync("./tokens.json").toString());
  console.log({ accessToken });
  urApi.accessToken = accessToken;

  // Save accessTokens to `tokens.json`
  Deno.writeTextFileSync("./tokens.json", JSON.stringify(accessToken));
} catch (_) {
  console.log("\nFailed to read ./tokens.json\n");
}

// Get initial request token
const requestToken: string = await urApi.getRequestToken();
console.log("getRequestToken", requestToken);

// Get player's accessToken, to make requests with
if (accessToken) {
  // Test accessTokens work still
  try {
    await callAPI("characters.getCharacters", {
      charactersIDs: 123,
    });
  } catch (_) {
    console.log("\naccessToken has expired\n");
    accessToken = undefined;
  }
}

// Wait for user to validate accessToken with link
if (!accessToken) {
  // Log URL to authorize your account
  const url = urApi.getAuthorizeUrl("about:new");
  console.log(url);

  // Wait for enter input
  alert("Press <ENTER> when ready");

  await urApi.getAccessToken();

  // Save accessTokens to `tokens.json`
  Deno.writeTextFileSync("./tokens.json", JSON.stringify(urApi.accessToken));
}
