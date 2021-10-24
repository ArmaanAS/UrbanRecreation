import { parentPort } from 'worker_threads';
import { GameGenerator } from '../Game';
import GameRenderer from '../utils/GameRenderer';
import Analysis from './Analysis';
let g;
let buffer = [];
function handleMessage() {
    while (buffer.length) {
        const data = buffer.splice(0, 1)[0];
        if (data.type == 'recreate') {
            const d = data;
            buffer = d.moves;
            handleMessage();
        }
        else if (data.type == 'init') {
            const d = data;
            g = GameGenerator.createUnique(d.h1, d.h2, d.life, d.pillz, d.name1, d.name2, d.first);
            GameRenderer.draw(g, true);
            parentPort === null || parentPort === void 0 ? void 0 : parentPort.postMessage('Update');
        }
        else if (data.type == 'move') {
            const d = data;
            g.select(d.index, d.pillz, d.fury || false);
            GameRenderer.draw(g, true);
            parentPort === null || parentPort === void 0 ? void 0 : parentPort.postMessage('Update');
        }
        else if (data.type == 'simulate') {
            const d = data;
            (async function () {
                const gc = g.clone();
                if (d.index != undefined) {
                    gc.select(d.index, 0, false);
                }
                const log = console.log;
                console.log = () => 0;
                console.time('a');
                const m = await Analysis.iterTree(gc);
                console.log = log;
                console.timeEnd('a');
                if (!g.round.first) {
                    m.turn = !m.turn;
                }
                const best = m.best();
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
}
parentPort === null || parentPort === void 0 ? void 0 : parentPort.on('message', function incoming(data) {
    buffer.push(data);
    console.log(`buffer: (${buffer.length})`, buffer);
    handleMessage();
});
//# sourceMappingURL=data:application/json;base64,eyJ2ZXJzaW9uIjozLCJmaWxlIjoiVGhyZWFkU2VydmVyLmpzIiwic291cmNlUm9vdCI6Ii9DOi9Vc2Vycy9TdHVkZW50L0RvY3VtZW50cy9Ob2RlSlNXb3Jrc3BhY2UvVXJiYW5SZWNyZWF0aW9uL3NyYy8iLCJzb3VyY2VzIjpbInRzL3NvbHZlci9UaHJlYWRTZXJ2ZXIudHMiXSwibmFtZXMiOltdLCJtYXBwaW5ncyI6IkFBQUEsT0FBTyxFQUFFLFVBQVUsRUFBRSxNQUFNLGdCQUFnQixDQUFDO0FBRTVDLE9BQWEsRUFBRSxhQUFhLEVBQUUsTUFBTSxTQUFTLENBQUM7QUFFOUMsT0FBTyxZQUFZLE1BQU0sdUJBQXVCLENBQUM7QUFDakQsT0FBTyxRQUFRLE1BQU0sWUFBWSxDQUFDO0FBRWxDLElBQUksQ0FBTyxDQUFDO0FBQ1osSUFBSSxNQUFNLEdBQWUsRUFBRSxDQUFDO0FBb0M1QixTQUFTLGFBQWE7SUFFcEIsT0FBTyxNQUFNLENBQUMsTUFBTSxFQUFFO1FBQ3BCLE1BQU0sSUFBSSxHQUFhLE1BQU0sQ0FBQyxNQUFNLENBQUMsQ0FBQyxFQUFFLENBQUMsQ0FBQyxDQUFDLENBQUMsQ0FBQyxDQUFDO1FBRTlDLElBQUksSUFBSSxDQUFDLElBQUksSUFBSSxVQUFVLEVBQUU7WUFDM0IsTUFBTSxDQUFDLEdBQUcsSUFBb0IsQ0FBQztZQUUvQixNQUFNLEdBQUcsQ0FBQyxDQUFDLEtBQUssQ0FBQztZQUNqQixhQUFhLEVBQUUsQ0FBQztTQUVqQjthQUFNLElBQUksSUFBSSxDQUFDLElBQUksSUFBSSxNQUFNLEVBQUU7WUFDOUIsTUFBTSxDQUFDLEdBQUcsSUFBZ0IsQ0FBQTtZQUMxQixDQUFDLEdBQUcsYUFBYSxDQUFDLFlBQVksQ0FDNUIsQ0FBQyxDQUFDLEVBQUUsRUFBRSxDQUFDLENBQUMsRUFBRSxFQUNWLENBQUMsQ0FBQyxJQUFJLEVBQUUsQ0FBQyxDQUFDLEtBQUssRUFDZixDQUFDLENBQUMsS0FBSyxFQUFFLENBQUMsQ0FBQyxLQUFLLEVBQ2hCLENBQUMsQ0FBQyxLQUFLLENBQ1IsQ0FBQztZ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