import Game, { Winner } from "../game/Game.ts";
import { shiftRange } from "../utils/Utils.ts";
// import Bar from "./Bar.ts"
import Minimax, { GameResult, Node } from "./Minimax.ts";
// import DistributedAnalysis from "./DistributedAnalysis.ts";
import { Turn } from "../game/types/Types.ts";

export default class Analysis {
  static async iterTree(
    game: Game,
    isRoot = true,
    rootName?: string,
    freeze = true,
  ) {
    const rootNode = new Minimax(rootName, game.turn);
    const rootGame = game.clone();
    let games = [rootGame];
    let nodes: Node[] = [rootNode];
    let indexes: number[] | undefined;

    let depth = 0;

    if (isRoot && rootGame.firstHasSelected) {
      indexes = [rootGame.deselect(false)!];
      rootNode.turn = rootGame.turn;
      rootNode.playingSecond = true;
    } else {
      indexes = rootGame.unplayedCardIndexes;
    }

    while (games.length) {
      const roundGames: Game[] = [];
      const roundNodes: Node[] = [];
      let counter = 0;

      for (let index = 0; index < games.length; index++) {
        const parentGame = games[index];
        const parentNode = nodes[index];

        if (indexes === undefined) {
          indexes = parentGame.unplayedCardIndexes;
        }

        const pillz = parentGame.playingPlayer.pillz;
        root: for (const i of indexes) {
          let breaking = false;
          card: for (const p of shiftRange(pillz)) {
            for (const f of (p <= pillz - 3 ? [true, false] : [false])) {
              counter++;

              const game = parentGame.clone();
              game.select(i, p, f);

              if (!game.firstHasSelected && game.winner !== Winner.PLAYING) {
                const node = parentNode.add(
                  `${i} ${p} ${f}`,
                  game.turn,
                  parentNode.playingSecond,
                );

                if (game.winner === Winner.PLAYER_1) {
                  node.result = GameResult.PLAYER_1_WIN;
                  if (!parentNode.defered) {
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
                  if (!parentNode.defered) {
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
                  parentNode.add(await Analysis.iterTree(game, false));
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

    return !isRoot && freeze ? rootNode.freeze() : rootNode;
  }

  static async iterTree2(
    game: Game,
    isRoot = true,
    rootName?: string,
    freeze = true,
  ) {
    const rootNode = new Minimax(rootName, game.turn);
    const rootGame = game.clone();
    let games = [rootGame];
    let nodes: Node[] = [rootNode];
    let indexes: number[] | undefined;

    let depth = 0;

    if (isRoot && rootGame.firstHasSelected) {
      indexes = [rootGame.deselect(false)!];
      rootNode.turn = rootGame.turn;
      rootNode.playingSecond = true;
    } else {
      indexes = rootGame.unplayedCardIndexes;
    }

    while (games.length) {
      const roundGames: Game[] = [];
      const roundNodes: Node[] = [];
      let counter = 0;

      for (let gameIndex = 0; gameIndex < games.length; gameIndex++) {
        const parentGame = games[gameIndex];
        const parentNode = nodes[gameIndex];

        if (indexes === undefined) {
          indexes = parentGame.unplayedCardIndexes;
        }

        const pillz = parentGame.playingPlayer.pillz;
        root: for (const i of indexes) {
          let breaking = false;
          card: for (const p of shiftRange(pillz)) {
            for (const f of (p <= pillz - 3 ? [true, false] : [false])) {
              counter++;

              const game = parentGame.clone();
              game.select(i, p, f);

              // TODO: Unwrap all these nested ifs

              if (!game.firstHasSelected && game.winner !== Winner.PLAYING) {
                const node = parentNode.add(
                  `${i} ${p} ${f}`,
                  game.turn,
                  parentNode.playingSecond,
                );

                if (game.winner === Winner.PLAYER_1) {
                  node.result = GameResult.PLAYER_1_WIN;
                  if (!parentNode.defered) {
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
                  if (!parentNode.defered) {
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
                  parentNode.add(await Analysis.iterTree(game, false));
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

    return !isRoot && freeze ? rootNode.freeze() : rootNode;
  }

  static iterTree3(
    game: Game,
    isRoot = true,
    rootName?: string,
    freeze = true,
    depth = 0,
  ): Node {
    const node = new Minimax(rootName, game.turn);

    if (isRoot && game.firstHasSelected) {
      const index = game.deselect(false)!;
      node.turn = game.turn;
      node.playingSecond = true;
      return this.processLevel(game, node, [index], depth);
    }

    return this.processLevel(
      game,
      node,
      game.unplayedCardIndexes,
      depth,
      freeze,
      isRoot,
    );
  }

  private static processLevel(
    game: Game,
    node: Node,
    indexes: number[],
    depth: number,
    freeze = true,
    isRoot = false,
  ): Node {
    console.info(`[${depth}] Processing level`);

    const pillz = game.playingPlayer.pillz;
    let breaking = false;

    root: for (const i of indexes) {
      card: for (const p of shiftRange(pillz)) {
        for (const f of (p <= pillz - 3 ? [true, false] : [false])) {
          const childGame = game.clone();
          childGame.select(i, p, f);

          if (
            !childGame.firstHasSelected && childGame.winner !== Winner.PLAYING
          ) {
            const childNode = node.add(
              `${i} ${p} ${f}`,
              childGame.turn,
              node.playingSecond,
            );

            switch (childGame.winner) {
              case Winner.PLAYER_1:
                childNode.result = GameResult.PLAYER_1_WIN;
                if (!node.defered) {
                  if (childNode.turn === Turn.PLAYER_1) {
                    childNode.break = true;
                    break root;
                  } else if (p === pillz) {
                    breaking = true;
                  } else if (breaking && f && p === pillz - 3) {
                    childNode.break = true;
                    break card;
                  }
                }
                break;
              case Winner.TIE:
                childNode.result = GameResult.TIE;
                break;
              case Winner.PLAYER_2:
                childNode.result = GameResult.PLAYER_2_WIN;
                if (!node.defered) {
                  if (childNode.turn === Turn.PLAYER_2) {
                    childNode.break = true;
                    break root;
                  } else if (p === pillz) {
                    breaking = true;
                  } else if (breaking && f && p === pillz - 3) {
                    childNode.break = true;
                    break card;
                  }
                }
                break;
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
              // Recursive call for next level
              this.processLevel(
                childGame,
                childNode,
                childGame.unplayedCardIndexes,
                depth + 1,
              );
            }
          }
        }
      }
    }

    return !isRoot && freeze ? node.freeze() : node;
  }
  // static async iterTree(
  //   game: Game,
  //   child = false,
  //   rootName?: string,
  //   freeze = true,
  // ) {
  //   const rootNode = new Minimax(rootName, game.turn);
  //   const rootAnalysis = new Analysis(game);

  //   // Handle already completed games
  //   if (game.winner !== Winner.PLAYING) {
  //     if (game.winner === Winner.PLAYER_1) {
  //       rootNode.result = GameResult.PLAYER_1_WIN;
  //     } else if (game.winner === Winner.TIE) {
  //       rootNode.result = GameResult.TIE;
  //     } else {
  //       rootNode.result = GameResult.PLAYER_2_WIN;
  //     }
  //     return rootNode;
  //   }

  //   let _i: number | undefined;
  //   if (!child && (_i = rootAnalysis.deselect()) !== undefined) {
  //     rootNode.turn = rootAnalysis.turn;
  //     rootNode.playingSecond = true;

  //     // Process single move for playing second
  //     const analysis = new Analysis(rootAnalysis.game);
  //     const game = analysis.game;
  //     game.select(_i, 0, false); // Use minimal resources for initial move
  //   }

  //   await this.processGameState(game, rootNode);

  //   return child && freeze ? rootNode.freeze() : rootNode;
  // }

  // private static async processGameState(
  //   game: Game,
  //   parentNode: Node,
  // ) {
  //   const analysis = new Analysis(game);
  //   const indexes = analysis.unplayedCardIndexes;
  //   const pillz = analysis.playingPlayer.pillz;

  //   // Process moves in order of potential effectiveness
  //   for (const i of indexes) {
  //     for (const p of shiftRange(pillz)) {
  //       for (const f of (p <= pillz - 3 ? [true, false] : [false])) {
  //         const moveAnalysis = new Analysis(analysis.game);
  //         const moveGame = moveAnalysis.game;
  //         moveGame.select(i, p, f);

  //         const node = parentNode.add(
  //           `${i} ${p} ${f}`,
  //           moveAnalysis.turn,
  //           parentNode.playingSecond,
  //         );

  //         if (
  //           !moveGame.firstHasSelected && moveGame.winner !== Winner.PLAYING
  //         ) {
  //           // Terminal state
  //           if (moveGame.winner === Winner.PLAYER_1) {
  //             node.result = GameResult.PLAYER_1_WIN;
  //             if (!parentNode.defered && node.turn === Turn.PLAYER_1) {
  //               return; // Early cutoff for winning move
  //             }
  //           } else if (moveGame.winner === Winner.TIE) {
  //             node.result = GameResult.TIE;
  //           } else {
  //             node.result = GameResult.PLAYER_2_WIN;
  //             if (!parentNode.defered && node.turn === Turn.PLAYER_2) {
  //               return; // Early cutoff for winning move
  //             }
  //           }
  //         } else {
  //           // Recursively analyze next moves
  //           const childNode = await this.iterTree(
  //             moveGame,
  //             true,
  //             undefined,
  //             false,
  //           );
  //           node.nodes.push(childNode);
  //         }

  //         // Clean up
  //         moveAnalysis.game = null as any;
  //       }
  //     }
  //   }
  // }
}
