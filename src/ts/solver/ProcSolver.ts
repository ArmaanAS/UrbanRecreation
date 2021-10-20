import cluster from 'cluster'
import { WorkerSolverData } from '../types/Types';
import Analysis from './Analysis';
import process from 'process'

if (cluster.isWorker) {
  const id = cluster.worker.id;

  const log = console.log;
  console.log = () => { };

  log(`[${id.toString().green}] ${'Thread Started'.grey}`);


  process.on('message', ({ msg }) => {
    log("Received initial message: " + msg);

    process.send!({ msg: "Thanks for having me" });

    process.on('message', async (data: WorkerSolverData) => {
      log(data.game.constructor.name);

      // let m = await Analysis.iterTree(Game.from(workerData), true);
      log(`[${data.id.toString().yellow}] ${'Analysis Started'.grey}`);
      const m = await Analysis.iterTree(data.game, true);

      log(`[${data.id.toString().yellow}] ${'Analysis Finished'.grey}`);
      process.send!(m);
    })
  })

  // await new Promise((resolve) => { });
}