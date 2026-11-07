import { localReads, type Reads } from "./reads";
import { planningTools } from "./planning";
import { calendarTools } from "./calendar";
import { studyTools } from "./study";

export function createQueryTools(read: Reads = localReads, now: () => number = Date.now) {
  return { ...planningTools(read), ...calendarTools(read, now), ...studyTools(read, now) };
}
export const queryTools = createQueryTools();
// No prototype members can accidentally become callable tools.
export const toolProvider: Record<string, (args: Record<string, unknown>) => Promise<unknown>> = Object.assign(Object.create(null),
  Object.fromEntries(Object.entries(queryTools).map(([name, tool]) => [name, tool.run])));
