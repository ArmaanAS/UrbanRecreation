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
    worker = new Worker("./solver/ThreadServer.js");
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
    ws.on('message', async function incoming(raw) {
        const data = JSON.parse(raw.toString());
        console.log('received:', data);
        if (data.type == 'init') {
            moves = [data];
            if (simulated || initialised) {
                recreate();
            }
            else {
                worker.postMessage(data);
            }
            initialised = true;
        }
        else if (data.type == 'move') {
            moves.push(data);
            if (simulated) {
                recreate();
            }
            else {
                worker.postMessage(data);
            }
        }
        else if (data.type == 'simulate') {
            simulated = true;
            worker.postMessage(data);
        }
    });
});
//# sourceMappingURL=data:application/json;base64,eyJ2ZXJzaW9uIjozLCJmaWxlIjoiU2VydmVyLmpzIiwic291cmNlUm9vdCI6Ii9DOi9Vc2Vycy9TdHVkZW50L0RvY3VtZW50cy9Ob2RlSlNXb3Jrc3BhY2UvVXJiYW5SZWNyZWF0aW9uL3NyYy8iLCJzb3VyY2VzIjpbInRzL3NvbHZlci9TZXJ2ZXIudHMiXSwibmFtZXMiOltdLCJtYXBwaW5ncyI6IkFBQUEsT0FBTyxTQUFTLE1BQU0sSUFBSSxDQUFDO0FBQzNCLE9BQU8sRUFBRSxNQUFNLEVBQUUsTUFBTSxnQkFBZ0IsQ0FBQztBQUV4QyxJQUFJLEtBQUssR0FBYSxFQUFFLENBQUM7QUFDekIsSUFBSSxNQUFjLENBQUM7QUFDbkIsSUFBSSxTQUFTLEdBQUcsS0FBSyxDQUFDO0FBQ3RCLElBQUksV0FBVyxHQUFHLEtBQUssQ0FBQztBQUV4QixNQUFNLEdBQUcsR0FBRyxJQUFJLFNBQVMsQ0FBQyxNQUFNLENBQUMsRUFBRSxJQUFJLEVBQUUsSUFBSSxFQUFFLENBQUMsQ0FBQztBQUNqRCxJQUFJLEdBQWMsQ0FBQztBQUVuQixTQUFTLFFBQVE7SUFDZixTQUFTLEdBQUcsS0FBSyxDQUFDO0lBQ2xCLElBQUksTUFBTSxJQUFJLFNBQVMsRUFBRTtRQUN2QixNQUFNLENBQUMsU0FBUyxFQUFFLENBQUM7S0FDcEI7SUFFRCxNQUFNLEdBQUcsSUFBSSxNQUFNLENBQUMsMEJBQTBCLENBQUMsQ0FBQztJQUNoRCxNQUFNLENBQUMsRUFBRSxDQUFDLFNBQVMsRUFBRSxDQUFDLElBQUksRUFBRSxFQUFFO1FBQzVCLE9BQU8sQ0FBQyxHQUFHLENBQUMsc0JBQXNCLEVBQUUsSUFBSSxDQUFDLENBQUM7UUFDMUMsSUFBSSxJQUFJLENBQUMsSUFBSSxFQUFFO1lBQ2IsR0FBRyxDQUFDLElBQUksQ0FBQyxJQUFJLENBQUMsU0FBUyxDQUFDLElBQUksQ0FBQyxDQUFDLENBQUM7U0FDaEM7SUFDSCxDQUFDLENBQUMsQ0FBQztJQUNILE1BQU0sQ0FBQyxFQUFFLENBQUMsTUFBTSxFQUFFLENBQUMsSUFBSSxFQUFFLEVBQUU7UUFDekIsSUFBSSxJQUFJLEtBQUssQ0FBQztZQUNaLE9BQU8sQ0FBQyxLQUFLLENBQUMsSUFBSSxLQUFLLENBQUMsaUNBQWlDLElBQUksRUFBRSxDQUFDLENBQUMsQ0FBQztJQUN0RSxDQUFDLENBQUMsQ0FBQztJQUVILElBQUksS0FBSyxDQUFDLE1BQU0sRUFBRTtRQUNoQixNQUFNLENBQUMsV0FBVyxDQUFDLEVBQUUsSUFBSSxFQUFFLFVBQVUsRUFBRSxLQUFLLEVBQUUsS0FBSyxFQUFFLENBQUMsQ0FBQztLQUN4RDtBQUNILENBQUM7QUFFRCxRQUFRLEVBQUUsQ0FBQztBQU1YLEdBQUcsQ0FBQyxFQUFFLENBQUMsWUFBWSxFQUFFLFNBQVMsVUFBVSxDQUFDLEVBQUU7SUFDekMsR0FBRyxHQUFHLEVBQUUsQ0FBQztJQUNULE9BQU8sQ0FBQyxHQUFHLENBQUMseUJBQXlCLENBQUMsQ0FBQztJQUN2QyxFQUFFLENBQUMsRUFBRSxDQUFDLFNBQVMsRUFBRSxLQUFLLFVBQVUsUUFBUSxDQUFDLEdBQUc7UUFDMUMsTUFBTSxJQUFJLEdBQVMsSUFBSSxDQUFDLEtBQUssQ0FBQyxHQUFHLENBQUMsUUFBUSxFQUFFLENBQUMsQ0FBQztRQUU5QyxPQUFPLENBQUMsR0FBRyxDQUFDLFdBQVcsRUFBRSxJQUFJLENBQUMsQ0FBQztRQUUvQixJQUFJLElBQUksQ0FBQyxJQUFJLElBQUksTUFBTSxFQUFFO1lBQ3ZCLEtBQUssR0FBRyxDQUFDLElBQUksQ0FBQyxDQUFDO1lBQ2YsSUFBSSxTQUFTLElBQUksV0FBVyxFQUFFO2dCQUM1QixRQUFRLEVBQUUsQ0FBQzthQUNaO2lCQUFNO2dCQUNMLE1BQU0sQ0FBQyxXQUFXLENBQUMsSUFBSSxDQUFDLENBQUM7YUFDMUI7WUFDRCxXQUFXLEdBQUcsSUFBSSxDQUFDO1NBRXBCO2FBQU0sSUFBSSxJQUFJLENBQUMsSUFBSSxJQUFJLE1BQU0sRUFBRTtZQUM5QixLQUFLLENBQUMsSUFBSSxDQUFDLElBQUksQ0FBQyxDQUFDO1lBQ2pCLElBQUksU0FBUyxFQUFFO2dCQUNiLFFBQVEsRUFBRSxDQUFDO2FBQ1o7aUJBQU07Z0JBQ0wsTUFBTSxDQUFDLFdBQVcsQ0FBQyxJQUFJLENBQUMsQ0FBQzthQUMxQjtTQUVGO2FBQU0sSUFBSSxJQUFJLENBQUMsSUFBSSxJQUFJLFVBQVUsRUFBRTtZQUNsQyxTQUFTLEdBQUcsSUFBSSxDQUFDO1lBQ2pCLE1BQU0sQ0FBQyxXQUFXLENBQUMsSUFBSSxDQUFDLENBQUM7U0FDMUI7SUFDSCxDQUFDLENBQUMsQ0FBQztBQUNMLENBQUMsQ0FBQyxDQUFDIn0=