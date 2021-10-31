import cluster from 'cluster'
import { counter } from '../CardBattle';
import { WorkerSolverData } from '../types/Types';
import Analysis from './Analysis';

if (cluster.isWorker) {
  const id = cluster.worker.id;
  const pid = `[${id.toString().blue}]`

  const log = console.log;
  console.log = () => 0;

  log(`${pid} ${'Process Started'.grey}`);

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
  let prevCount = 0,
    prevTime = +new Date();
  setInterval(() => {
    const t = +new Date();
    const dt = (t - prevTime) / 1000;
    const dcount = counter - prevCount;
    if (dcount > 0) {
      const rate = Math.floor(dcount / dt);
      const srate = rate < 1000 ? rate :
        rate < 1e6 ? `${rate / 1000}k` : `${rate / 1000000}M`;
      const msg = ` ${srate} CardBattles/sec `;

      if (rate < 100)
        log(`${pid} ${msg.bgRed.white}\n`)
      else if (rate < 5000)
        log(`${pid} ${msg.bgYellow.white}\n`)
      else if (rate < 15000)
        log(`${pid} ${msg.bgBlue.white}\n`)
      else if (rate < 30000)
        log(`${pid} ${msg.bgMagenta.white}\n`)
      else
        log(`${pid} ${msg.bgGreen.white}\n`)
    }

    prevCount = counter;
    prevTime = t;
  }, 5000)
}