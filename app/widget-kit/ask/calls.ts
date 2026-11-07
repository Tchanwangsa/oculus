import { request } from "./pending";
import type { JsonSchema } from "./schema";

const isObject = (v: unknown): v is Record<string, unknown> => !!v && typeof v === "object" && !Array.isArray(v);

/** One model call answering JSON that matches `schema`. The parent checks
 *  the student's Allow, the cap and the sizes; Rust checks the schema and
 *  the answer. A wrong argument rejects here, before anything is posted. */
export function ask<T = unknown>(prompt: string, schema: JsonSchema): Promise<T> {
  if (typeof prompt !== "string") return Promise.reject(new TypeError("ask: the prompt is a string"));
  if (!isObject(schema)) return Promise.reject(new TypeError("ask: the schema is a JSON Schema object"));
  let plain: Record<string, unknown>;
  try {
    // Only JSON crosses: a function or a cycle fails here, not in postMessage.
    plain = JSON.parse(JSON.stringify(schema));
  } catch (e) {
    return Promise.reject(new TypeError(`ask: the schema isn't JSON (${e instanceof Error ? e.message : String(e)})`));
  }
  return request({ kind: "ask", prompt, schema: plain }) as Promise<T>;
}

/** `text` as the student's next message in the chat. */
export async function send(text: string): Promise<void> {
  if (typeof text !== "string") throw new TypeError("send: the text is a string");
  await request({ kind: "send", text });
}
