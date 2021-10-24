import cluster from 'cluster'
import { WorkerSolverData } from '../types/Types';
import Analysis from './Analysis';

if (cluster.isWorker) {
  const id = cluster.worker.id;

  const log = console.log;
  console.log = () => 0;

  log(`[${id.toString().blue}] ${'Process Started'.grey}`);



  process.send?.({ msg: "ready" });

  // process.once("message", ({ msg }) => {
  //   log("Received initial message: " + msg);

  //   process.send!({ msg: "Thanks for having me" });

  process.on('message', async (data: WorkerSolverData) => {
    // if (Math.random() < 0.05) process.exit(1);
    // let m = await Analysis.iterTree(Game.from(workerData), true);
    log(`[${data.id.toString().yellow}] ${'Analysis Started'.grey}`);
    const m = await Analysis.iterTree(data.game, true);

    log(`[${data.id.toString().yellow}] ${'Analysis Finished'.grey}`);
    process.send?.(m);
  })
  // })


  // process.on("exit", () => {
  //   log(`[${id.toString().blue}] ${'Process exit'.red}`);
  //   process.exit(0);
  // })
  // process.on("SIGINT", () => {
  //   log(`[${id.toString().blue}] ${'Process SIGINT'.red}`);
  //   process.exit(0);
  // })
}