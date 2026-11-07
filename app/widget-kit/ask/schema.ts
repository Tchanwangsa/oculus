/** The subset of JSON Schema `ask` takes; `widget.d.ts` declares the same
 *  type for widgets, and Rust refuses anything outside it
 *  (`src-tauri/src/widgets/ask/schema/`). */
export type JsonType = "object" | "array" | "string" | "number" | "integer" | "boolean" | "null";

export interface JsonSchema {
  type?: JsonType | JsonType[];
  properties?: Record<string, JsonSchema>;
  required?: string[];
  additionalProperties?: boolean;
  items?: JsonSchema;
  enum?: unknown[];
  const?: unknown;
  minItems?: number;
  maxItems?: number;
  minimum?: number;
  maximum?: number;
  minLength?: number;
  maxLength?: number;
  description?: string;
  title?: string;
}
