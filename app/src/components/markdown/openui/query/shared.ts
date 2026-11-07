import { z } from "zod/v4";
import type { Subject } from "@/lib/db";

export const MAX_ROWS = 400;
export const limit = z.number().int().positive().max(Number.MAX_SAFE_INTEGER).default(100).transform(n => Math.min(n, MAX_ROWS));
export const subject = z.union([z.number().int().positive(), z.string().trim().min(1).max(200)]).optional();
export const projectEvent = "oculus:projects-updated";
export const calendarEvent = "oculus:calendar-updated";
export const revisionEvent = "oculus:revision-updated";
export const subjectEvents = ["oculus:subjects-updated", calendarEvent, "oculus:file-scraped"];
export type QueryTool = { events: readonly string[]; run: (args: Record<string, unknown>) => Promise<unknown[]> };

export function tool<S extends z.ZodRawShape>(schema: z.ZodObject<S>, events: readonly string[], read: (args: z.infer<z.ZodObject<S>>) => Promise<unknown[]>): QueryTool {
  return { events, run: async args => {
    const parsed = schema.parse(args);
    return (await read(parsed)).slice(0, Number((parsed as { limit?: number }).limit ?? MAX_ROWS));
  } };
}

/** CLI subject codes may omit the term suffix, but single-subject reads refuse ambiguity. */
export function subjectIds(rows: Subject[], wanted?: string | number, single = false): number[] | undefined {
  if (wanted === undefined) return undefined;
  const fold = (s: string) => s.toUpperCase().replace(/[^A-Z0-9]+/g, "_").replace(/^_|_$/g, "");
  const code = typeof wanted === "string" ? fold(wanted) : "";
  const matches = rows.filter(row => typeof wanted === "number" ? row.id === wanted : fold(row.code) === code || fold(row.code).startsWith(`${code}_`));
  if (!matches.length) throw new Error(`Unknown subject: ${wanted}`);
  if (single && matches.length !== 1) throw new Error(`Ambiguous subject: ${wanted}; use the full code or id`);
  return matches.map(row => row.id);
}
