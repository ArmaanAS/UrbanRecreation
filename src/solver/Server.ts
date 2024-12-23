import WebSocket from "ws";
import { Worker } from "node:worker_threads";

let moves: object[] = [];
let worker: Worker;
let simulated = false;
let initialised = false;

const wss = new WebSocket.Server({ port: 3002 });
let gws: WebSocket;

function recreate() {
  simulated = false;
  if (worker !== undefined) {
    worker.terminate();
  }

  worker = new Worker("./solver/ThreadServer.js");
  worker.on("message", (data) => {
    console.log("postMessage -> Main:");
    console.dir(data, { depth: 2 });

    if (data.type) {
      gws.send(JSON.stringify(data));
    }
  });
  worker.on("exit", (code) => {
    if (code > 1) {
      console.error(new Error(`Worker stopped with exit code ${code}`));
    }
  });

  if (moves.length) {
    worker.postMessage({ type: "recreate", moves: moves });
  }
}

recreate();

interface Data {
  type: string;
  battleTurnPlayerID: number;
  battleRound: number;
}

// let prevPacket: any;
wss.on("connection", function connection(ws) {
  gws = ws;
  console.log("Connection established!");
  ws.on("message", function incoming(raw) {
    const data: Data = JSON.parse(raw.toString());

    // if (prevPacket !== undefined &&
    //   data.type == 'init' &&
    //   prevPacket.battleRound == data.battleRound &&
    //   prevPacket.battleTurnPlayerID == data.battleTurnPlayerID)
    //   return;

    console.log("received type:", data.type);

    if (data.type == "init") {
      moves = [data];
      if (simulated || initialised) {
        recreate();
      } else {
        worker.postMessage(data);
      }
      initialised = true;
    } else if (data.type == "move") {
      moves.push(data);
      if (simulated) {
        recreate();
      } else {
        worker.postMessage(data);
      }
    } else if (data.type == "simulate") {
      simulated = true;
      worker.postMessage(data);
    }

    // prevPacket = data;
  });
});
