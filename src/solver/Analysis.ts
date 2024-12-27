import Game, { Winner } from "../game/Game.ts";
import { shiftRange } from "../utils/Utils.ts";
import Minimax, { GameResult, Node } from "./Minimax.ts";
import { Turn } from "../game/types/Types.ts";

export default class Analysis {
  static iterTreeWorker(
    game: Game,
  ): Promise<Minimax | undefined> {
    const url = new URL("worker.ts", import.meta.url);
    const worker = new Worker(url, {
      type: "module",
      deno: {
        permissions: "inherit",
      },
    });

    worker.postMessage({ game });

    return new Promise((res) => {
      const abort = () => {
        console.info("Terminating worker");
        worker.terminate();
        res(undefined);
      };
      // console.info("Adding SIGINT listener");
      Deno.addSignalListener("SIGINT", abort);

      worker.onmessage = (e) => {
        // console.info("Removing SIGINT listener");
        Deno.removeSignalListener("SIGINT", abort);
        res(e.data as Minimax);
        worker.terminate();
      };
    });
  }

  static iterTree(
    game: Game,
    isRoot = true,
  ): Minimax {
    const rootNode = new Minimax(undefined, game.turn);
    const rootGame = game.clone();
    let games = [rootGame];
    let nodes: Node[] = [rootNode];

    let depth = 0;

    let indexes: number[] | undefined;
    if (isRoot && rootGame.firstHasSelected) {
      indexes = [rootGame.deselect(false)!];
      rootNode.turn = rootGame.turn;
      rootNode.playingSecond = true;
    }

    while (games.length) {
      const roundGames: Game[] = [];
      const roundNodes: Node[] = [];
      let counter = 0;

      // while (games.length) {
      //   const parentGame = games.pop()!;
      //   const parentNode = nodes.pop()!;
      // for (let index = 0; index < games.length; index++) {
      for (let index = games.length - 1; index >= 0; index--) {
        const parentGame = games[index];
        const parentNode = nodes[index];
        // games.length--;
        // nodes.length--;

        indexes ??= parentGame.unplayedCardIndexes;

        const pillz = parentGame.playingPlayer.pillz;
        root: for (const i of indexes) {
          let breaking = false;
          card: for (const p of shiftRange(pillz)) {
            for (const f of (p <= pillz - 3 ? [true, false] : [false])) {
              counter++;

              const game = parentGame.clone();
              game.select(i, p, f, false);

              if (!game.firstHasSelected && game.winner !== Winner.PLAYING) {
                const node = parentNode.add(
                  `${i} ${p} ${f}`,
                  game.turn,
                  parentNode.playingSecond,
                );

                if (game.winner === Winner.PLAYER_1) {
                  node.result = GameResult.PLAYER_1_WIN;
                  if (!parentNode.deferred) {
                    if (node.turn === Turn.PLAYER_1) {
                      node.break = true;
                      break root;
                    } else if (p === pillz) {
                      breaking = true;
                    } else if (breaking && f && p === pillz - 3) {
                      node.break = true;
                      break card;
                    }
                  }
                } else if (game.winner === Winner.TIE) {
                  node.result = GameResult.TIE;
                } else if (game.winner === Winner.PLAYER_2) {
                  node.result = GameResult.PLAYER_2_WIN;
                  if (!parentNode.deferred) {
                    if (node.turn === Turn.PLAYER_2) {
                      node.break = true;
                      break root;
                    } else if (p === pillz) {
                      breaking = true;
                    } else if (breaking && f && p === pillz - 3) {
                      node.break = true;
                      break card;
                    }
                  }
                }
              } else {
                if (game.round === 1 && !game.firstHasSelected) {
                  parentNode.add(Analysis.iterTree(game, false));
                } else {
                  const node = parentNode.add(
                    `${i} ${p} ${f}`,
                    game.turn,
                    parentNode.playingSecond,
                  );
                  roundGames.push(game);
                  roundNodes.push(node);
                }
              }
            }
          }
        }

        indexes = undefined;
      }

      console.info(
        `[${depth++}] Finished  Moves: ${counter}`,
      );
      games = roundGames;
      nodes = roundNodes;
    }

    return isRoot ? rootNode : rootNode.freeze();
  }

  static iterTree3(
    game: Game,
    isRoot = true,
    rootName?: string,
    freeze = true,
  ): Minimax {
    const node = new Minimax(rootName, game.turn);
    const rootGame = game.clone();

    if (isRoot && rootGame.firstHasSelected) {
      const index = rootGame.deselect(false)!;
      node.turn = rootGame.turn;
      node.playingSecond = true;
      this.processRound(rootGame, node, [index]);
    } else {
      this.processRound(rootGame, node, rootGame.unplayedCardIndexes);
    }

    return isRoot && freeze ? node.freeze() : node;
  }

  private static processRound<N extends Node>(
    game: Game,
    node: N,
    cardIndexes: number[],
  ): N {
    const pillz = game.playingPlayer.pillz;
    let breaking = false;

    outer: for (const i of cardIndexes) {
      for (const p of shiftRange(pillz)) {
        for (const f of (p <= pillz - 3 ? [true, false] : [false])) {
          const childGame = game.clone();
          childGame.select(i, p, f);

          if (!childGame.firstHasSelected && childGame.hasWinner()) {
            const childNode = node.add(
              `${i} ${p} ${f}`,
              childGame.turn,
              node.playingSecond,
            );

            if (childGame.winner === Winner.PLAYER_1) {
              childNode.result = GameResult.PLAYER_1_WIN;
              if (node.deferred) continue;

              if (childNode.turn === Turn.PLAYER_1) {
                childNode.break = true;
                break outer;
              } else if (p === pillz) {
                breaking = true;
              } else if (breaking && f && p === pillz - 3) {
                childNode.break = true;
                continue outer;
              }
            } else if (childGame.winner === Winner.TIE) {
              childNode.result = GameResult.TIE;
            } else if (childGame.winner === Winner.PLAYER_2) {
              childNode.result = GameResult.PLAYER_2_WIN;
              if (node.deferred) continue;

              if (childNode.turn === Turn.PLAYER_2) {
                childNode.break = true;
                break outer;
              } else if (p === pillz) {
                breaking = true;
              } else if (breaking && f && p === pillz - 3) {
                childNode.break = true;
                continue outer;
              }
            }
          } else {
            if (childGame.round === 1 && !childGame.firstHasSelected) {
              node.add(Analysis.iterTree3(childGame, false));
            } else {
              const childNode = node.add(
                `${i} ${p} ${f}`,
                childGame.turn,
                node.playingSecond,
              );
              // Recursive call for next round
              this.processRound(
                childGame,
                childNode,
                childGame.unplayedCardIndexes,
              );
            }
          }
        }
      }
    }

    return node;
  }
}
