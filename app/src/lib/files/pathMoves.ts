/** Paths cited before the library moved into the Drive tree, or before a
 *  drawing was renamed from `.canvas` (a chat's tool output, an undo
 *  snapshot, a saved tab), followed through `path_moves`:
 *  the longest old prefix a path starts with, at a path boundary, with the
 *  rest carried over. `resolve_moved` in
 *  `app/src-tauri/src/library/migrate/resolve.rs` is the same rule in Rust;
 *  docs/library-move.md has the move. */
import { getDb } from "@/lib/db/connection";

export interface PathMove {
  old_prefix: string;
  new_prefix: string;
}

/** What may follow a prefix for it to match: a deeper path, a `#page=`
 *  anchor, a `:L15` line, a query. */
const BOUNDARY = new Set(["/", "#", ":", "?"]);

/** Where `path` went under `moves`, or null when no rule covers it. */
export function movedPath(path: string, moves: readonly PathMove[]): string | null {
  let best: PathMove | null = null;
  for (const move of moves) {
    const old = move.old_prefix;
    const covers = path === old || (path.startsWith(old) && BOUNDARY.has(path.charAt(old.length)));
    if (covers && (!best || old.length > best.old_prefix.length)) best = move;
  }
  return best ? best.new_prefix + path.slice(best.old_prefix.length) : null;
}

// Written at launch, before the window loads, and by Drive's reconcile when
// it renames a `.canvas` drawing, which it announces as a `drive-changed`
// move: read once, and again after a move (`forgetPathMoves`).
let moves: Promise<PathMove[]> | null = null;

/** The next lookup reads `path_moves` again. */
export function forgetPathMoves(): void {
  moves = null;
}

/** Where an old library path went, or null when nothing moved it. */
export function resolveMoved(path: string): Promise<string | null> {
  moves ??= getDb()
    .then((db) => db.select<PathMove[]>("SELECT old_prefix, new_prefix FROM path_moves"))
    .catch((error) => {
      moves = null;
      throw error;
    });
  return moves.then((list) => movedPath(path, list));
}
