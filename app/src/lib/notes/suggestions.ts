import { invoke } from "@tauri-apps/api/core";

import { getSetting, setSetting } from "@/lib/db";

/** Text to insert at the caret of the note at `path`, from the
 *  `documentSuggestions` job's model; `""` for none. A higher `requestId`
 *  supersedes a lower one, which then resolves `""`. */
export function suggestDocument(req: {
  requestId: number;
  path: string;
  before: string;
  after: string;
}): Promise<string> {
  return invoke<string>("document_suggest", req);
}

/** Drop the suggestion in flight, if any. */
export function cancelDocumentSuggestion(): Promise<void> {
  return invoke("document_suggest_cancel");
}

const SUGGESTIONS_KEY = "document_suggestions_enabled";

/** Off unless the student turned it on: every suggestion is a model turn. */
export async function getDocumentSuggestions(): Promise<boolean> {
  return (await getSetting(SUGGESTIONS_KEY)) === "1";
}

export async function setDocumentSuggestions(on: boolean): Promise<void> {
  await setSetting(SUGGESTIONS_KEY, on ? "1" : "0");
}
