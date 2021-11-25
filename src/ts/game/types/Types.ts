import Game from "../Game";

export interface WorkerSolverData {
  id: number;
  game: Game;
  child?: boolean;
  rootName?: string;
  freeze?: boolean;
}

export enum Turn {
  PLAYER_1 = 1,
  PLAYER_2 = 0
}