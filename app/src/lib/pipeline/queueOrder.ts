/**
 * Pushing a waiting file ahead of the others. `pushed` lists the paths the
 * student has already prioritised, oldest press first, so a second press goes
 * behind the first and every file never pressed stays behind both, in the
 * order it arrived. A job that has left the queue (started or dropped) falls
 * out of `pushed` here.
 */
export function prioritiseJob<T extends { relativePath: string }>(
  queue: readonly T[],
  pushed: readonly string[],
  path: string,
): { queue: T[]; pushed: string[] } | null {
  if (!queue.some((job) => job.relativePath === path)) return null;
  const present = new Set(queue.map((job) => job.relativePath));
  const order = [...pushed.filter((p) => p !== path && present.has(p)), path];
  const byPath = new Map(queue.map((job) => [job.relativePath, job]));
  const front = order.map((p) => byPath.get(p) as T);
  const rest = queue.filter((job) => !order.includes(job.relativePath));
  return { queue: [...front, ...rest], pushed: order };
}
