import { parentPort } from 'worker_threads';
import Game from '../Game';
import Analysis from './Analysis';
let g;
let buffer = [];
let running = false;
function handleMessage() {
    running = true;
    while (buffer.length) {
        let data = buffer.splice(0, 1)[0];
        if (data.type == 'recreate') {
            buffer = data.moves;
            handleMessage();
        }
        else if (data.type == 'init') {
            g = Game.createUnique(data.h1, data.h2, data.life, data.pillz, data.name1, data.name2, data.first);
            g.log(true);
            parentPort === null || parentPort === void 0 ? void 0 : parentPort.postMessage('Update');
        }
        else if (data.type == 'move') {
            g.select(data.index, data.pillz, data.fury || false);
            g.log(true);
            parentPort === null || parentPort === void 0 ? void 0 : parentPort.postMessage('Update');
        }
        else if (data.type == 'simulate') {
            (async function () {
                let gc = g.clone();
                if (data.index != undefined) {
                    gc.select(data.index, 0, false);
                }
                let log = console.log;
                console.log = () => { };
                console.time('a');
                let m = await Analysis.iterTree(gc);
                console.log = log;
                console.timeEnd('a');
                if (!g.round.first) {
                    m.turn = !m.turn;
                }
                let best = m.best();
                console.log(best.toString());
                parentPort === null || parentPort === void 0 ? void 0 : parentPort.postMessage({
                    type: 'play',
                    index: best.index,
                    pillz: best.pillz,
                    fury: best.fury
                });
            })();
        }
        else
            console.log('Unknown data:', data);
    }
    running = false;
}
parentPort === null || parentPort === void 0 ? void 0 : parentPort.on('message', function incoming(data) {
    buffer.push(data);
    console.log(`buffer: (${buffer.length})`, buffer);
    handleMessage();
});
//# sourceMappingURL=data:application/json;base64,eyJ2ZXJzaW9uIjozLCJmaWxlIjoiVGhyZWFkU2VydmVyLmpzIiwic291cmNlUm9vdCI6Ii9DOi9Vc2Vycy9TdHVkZW50L0RvY3VtZW50cy9Ob2RlSlNXb3Jrc3BhY2UvVXJiYW5SZWNyZWF0aW9uL3NyYy8iLCJzb3VyY2VzIjpbInRzL3NvbHZlci9UaHJlYWRTZXJ2ZXIudHMiXSwibmFtZXMiOltdLCJtYXBwaW5ncyI6IkFBQUEsT0FBTyxFQUVMLFVBQVUsRUFHWCxNQUFNLGdCQUFnQixDQUFDO0FBRXhCLE9BQU8sSUFBSSxNQUFNLFNBQVMsQ0FBQztBQUMzQixPQUFPLFFBQVEsTUFBTSxZQUFZLENBQUM7QUFFbEMsSUFBSSxDQUFPLENBQUM7QUFDWixJQUFJLE1BQU0sR0FBVSxFQUFFLENBQUM7QUFDdkIsSUFBSSxPQUFPLEdBQUcsS0FBSyxDQUFDO0FBRXBCLFNBQVMsYUFBYTtJQUNwQixPQUFPLEdBQUcsSUFBSSxDQUFDO0lBQ2YsT0FBTyxNQUFNLENBQUMsTUFBTSxFQUFFO1FBQ3BCLElBQUksSUFBSSxHQUFHLE1BQU0sQ0FBQyxNQUFNLENBQUMsQ0FBQyxFQUFFLENBQUMsQ0FBQyxDQUFDLENBQUMsQ0FBQyxDQUFDO1FBRWxDLElBQUksSUFBSSxDQUFDLElBQUksSUFBSSxVQUFVLEVBQUU7WUFDM0IsTUFBTSxHQUFHLElBQUksQ0FBQyxLQUFLLENBQUM7WUFDcEIsYUFBYSxFQUFFLENBQUM7U0FFakI7YUFBTSxJQUFJLElBQUksQ0FBQyxJQUFJLElBQUksTUFBTSxFQUFFO1lBQzlCLENBQUMsR0FBRyxJQUFJLENBQUMsWUFBWSxDQUNuQixJQUFJLENBQUMsRUFBRSxFQUFFLElBQUksQ0FBQyxFQUFFLEVBQ2hCLElBQUksQ0FBQyxJQUFJLEVBQUUsSUFBSSxDQUFDLEtBQUssRUFDckIsSUFBSSxDQUFDLEtBQUssRUFBRSxJQUFJLENBQUMsS0FBSyxFQUN0QixJQUFJLENBQUMsS0FBSyxDQUNYLENBQUM7WUFDRixDQUFDLENBQUMsR0FBRyxDQUFDLElBQUksQ0FBQyxDQUFDO1lBQ1osVUFBVSxh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