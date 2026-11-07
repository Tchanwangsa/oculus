/** How much a widget's request may carry. `MAX_PROMPT` and `MAX_SCHEMA`
 *  are checked again in Rust (`src-tauri/src/widgets/ask/prompt.rs`). */
export const MAX_PROMPT = 20_000;
export const MAX_SCHEMA = 8_000;
export const MAX_TEXT = 4_000;
/** Requests (asks and sends together) one allowance covers. */
export const ASK_CAP = 20;

/** What a refused request rejects with in the widget. */
export const REFUSED = {
  busy: "A request is already running.",
  denied: "The student didn't allow this widget to use a model.",
  capped: `This widget has used its ${ASK_CAP} requests.`,
  noChat: "The chat can't take a message now: a reply is running, or this widget isn't in a conversation.",
  emptyPrompt: "The prompt is empty.",
  longPrompt: `The prompt is over ${MAX_PROMPT.toLocaleString("en")} characters.`,
  longSchema: `The schema is over ${MAX_SCHEMA.toLocaleString("en")} characters of JSON.`,
  emptyText: "The message is empty.",
  longText: `The message is over ${MAX_TEXT.toLocaleString("en")} characters.`,
} as const;
