import WebSocket from 'ws';
import { Worker } from "worker_threads";

let moves = [];
let worker;
let simulated = false;
let initialised = false;

const wss = new WebSocket.Server({ port: 3002 });
let gws;

function recreate() {
  simulated = false;
  if (worker != undefined) {
    worker.terminate();
  }

  worker = new Worker("./threadServer.mjs");
  worker.on("message", (data) => {
    console.log('postMessage -> Main:', data);
    if (data.type) {
      gws.send(JSON.stringify(data));
    }
  });
  worker.on("exit", (code) => {
    if (code !== 0)
      console.error(new Error(`Worker stopped with exit code ${code}`));
  });

  if (moves.length) {
    worker.postMessage({ type: 'recreate', moves: moves });
  }
}

recreate();

wss.on('connection', function connection(ws) {
  gws = ws;
  console.log('Connection established!');
  ws.on('message', async function incoming(data) {
    data = JSON.parse(data);

    console.log('received:', data);

    if (data.type == 'init') {
      moves = [data];
      if (simulated || initialised) {
        recreate();
      } else {
        worker.postMessage(data);
      }
      initialised = true;

    } else if (data.type == 'move') {
      moves.push(data);
      if (simulated) {
        recreate();
      } else {
        worker.postMessage(data);
      }

    } else if (data.type == 'simulate') {
      simulated = true;
      worker.postMessage(data);
    }
  });
});