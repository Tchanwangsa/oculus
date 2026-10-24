import type { ItemInput } from "./types";
/** Choice answers are option text; numeric tolerances are absolute. */
export function validateItem(item: ItemInput): void {
  if (!item.prompt.trim() || !item.answer.trim())
    throw new Error("Questions need a prompt and model answer.");
  if (
    item.kind === "numeric" &&
    (!Number.isFinite(Number(item.answer)) ||
      (item.tolerance != null &&
        (!Number.isFinite(item.tolerance) || item.tolerance < 0)))
  )
    throw new Error(
      "Numeric answers need a finite number and nonnegative tolerance.",
    );
  if (
    item.kind === "choice" &&
    (!item.options ||
      item.options.length < 2 ||
      item.options.some((o) => !o.trim()) ||
      item.options.filter((o) => o === item.answer).length !== 1)
  )
    throw new Error(
      "Choice questions need at least two options and exactly one matching model answer.",
    );
  if (
    item.rubric?.some(
      (p) => !p.point.trim() || !Number.isFinite(p.marks) || p.marks <= 0,
    )
  )
    throw new Error("Rubric points need text and positive marks.");
}
