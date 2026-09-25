import UROAuth from "urban-rivals-oauth";

// OAuth access token saved by the authorization step below. It is a gitignored secret, so
// it is read at runtime rather than imported: a static import makes every type-check of
// this file fail on a clone that has never authorized, and a missing token is exactly the
// case the authorization step exists to handle. It is resolved against this file, as the
// import was, so the working directory does not matter.
const TOKENS_PATH = new URL("../tokens.json", import.meta.url);

function readAccessToken(): unknown {
  let text: string;
  try {
    text = Deno.readTextFileSync(TOKENS_PATH);
  } catch (error) {
    if (!(error instanceof Deno.errors.NotFound)) throw error;
    console.log(
      "\nNo tokens.json in the repo root yet; authorize below to create it (gitignored).\n",
    );
    return undefined;
  }
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new Error(
      `tokens.json is not valid JSON; delete it and re-run to authorize again`,
      { cause: error },
    );
  }
}

export const urApi = new UROAuth({
  key: Deno.env.get("API_KEY"),
  secret: Deno.env.get("API_SECRET"),
});

const accessToken = readAccessToken();
if (accessToken !== undefined) urApi.accessToken = accessToken;

export default async function callAPI(method: string, params: object) {
  const { items, context } = await urApi.query(method, params);

  return { items, context };
}

// Get initial request token
const requestToken: string = await urApi.getRequestToken();
console.log("getRequestToken", requestToken);

// Test accessTokens work still
let isTokenValid = accessToken !== undefined;
if (isTokenValid) {
  try {
    await callAPI("characters.getCharacters", {
      charactersIDs: 123,
    });
  } catch (_) {
    console.log("\naccessToken has expired\n");
    isTokenValid = false;
  }
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
  Deno.writeTextFileSync(TOKENS_PATH, JSON.stringify(urApi.accessToken));
}
