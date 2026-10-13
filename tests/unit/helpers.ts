import { type Board, SIZE, createBoard, idx } from '../../src/game/board';

/** Builds a board from 8 strings where '#' is a filled cell. */
export function boardFrom(rows: string[]): Board {
  const b = createBoard();
  rows.forEach((line, r) => {
    for (let c = 0; c < SIZE; c++) if (line[c] === '#') b[idx(r, c)] = 1;
  });
  return b;
}
