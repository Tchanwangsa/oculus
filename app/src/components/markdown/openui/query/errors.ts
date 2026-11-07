import type { OpenUIError } from "@openuidev/react-lang";

/** Failed reads can also leave dependent props empty; Retry owns that entire state. */
export function isFenceFailure(errors: OpenUIError[]) {
  return !errors.some(error => error.source === "query") && errors.some(error => error.severity !== "warning");
}
