import Game from "../Game";

export interface WorkerSolverData {
  id: number;
  game: Game;
}

// export type Turn = boolean
// export const Turn = {
//   PLAYER_1: true,
//   PLAYER_2: false,
// } as const
export enum Turn {
  PLAYER_1 = 1,
  PLAYER_2 = 0
}