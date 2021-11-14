import {
  isMainThread,
  MessagePort,
  parentPort,
  threadId
} from 'worker_threads';
import { WorkerSolverData } from '../game/types/Types';
import Analysis from './Analysis';

if (!isMainThread) {
  const log = console.log;
  console.log = () => 0;

  process.stdout.write("Test output: " + threadId + "\n");
  log(`[${threadId.toString().green}] ${'Thread Started'.grey}`);


  parentPort!.once('message', (data: { port: MessagePort }) => {
    log("Received initial MessagePort")
    const port = data.port;

    port.postMessage("Thanks for having me");

    port.on('message', async (data: WorkerSolverData) => {
      log(data.game.constructor.name);

      // let m = await Analysis.iterTree(Game.from(workerData), true);
      log(`[${data.id.toString().yellow}] ${'Analysis Started'.grey}`);
      const m = await Analysis.iterTree(data.game, true);

      log(`[${data.id.toString().yellow}] ${'Analysis Finished'.grey}`);
      port.postMessage(m);
    })
  })
}