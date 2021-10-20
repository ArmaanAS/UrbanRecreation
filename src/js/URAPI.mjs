const UROAuth = require('urban-rivals-oauth');

const auth = {
  request: "https://www.urban-rivals.com/api/auth/request_token.php",
  access: "http://www.urban-rivals.com/api/auth/access_token.php",
  key: "eb70e322189e1bdea45ccea251ed06d805f34bcba",
  secret: "a4303ab3b25f1d5b476e2d70df93b8a8",
  api: "https://www.urban-rivals.com/api/",
};

const urApi = new UROAuth({
  key: auth.key,
  secret: auth.secret
});

// requestToken = {
//   token: '18b5ee88c5c7399b401c1d5c9a02c4ee05f35d26d',
//   secret: 'b654ec979bb82b13562032760e9af82d'
// }


function query() {
  console.log("querying");
  urApi.getAccessToken()
    .then(a => {
      console.log("getAccessToken", a);

      urApi.query('urc.getCharacters', {})
        .then(a => {
          console.log("query", a);
        }).catch(err => {
          console.error("query", err);
        });

    }).catch(err => {
      console.error("getAccessToken", err);
    });
}

urApi.getRequestToken().then(a => {
  console.log('getRequestToken', a);

  const url = urApi.getAuthorizeUrl('https://localhost/');
  console.log(url);
});


let stdin = process.openStdin();

stdin.addListener("data", d => {
  console.log("your input: " + d.toString());

  query();
});

let a, b;

function callAPI(method, params) {
  urApi.query(method, params)
    .then(({
      items,
      context
    }) => {
      a = items;
      b = context;
      console.log(items, context);
    }).catch(e => {
      console.log(e);
    });
}
// callAPI("characters.getCharacters", {
//   charactersIDs: 123,
// });