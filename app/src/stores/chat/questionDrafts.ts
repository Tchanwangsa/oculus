/** Unsubmitted question answers, keyed by thread, request and native question.
 *  Only these drafts use localStorage; Rust owns submitted answers. */
export interface QuestionDraft {
  selected: string[];
  text: string;
  dismissed: boolean;
}
export type QuestionDrafts = Record<number, Record<number, Record<string, QuestionDraft>>>;
const KEY = "oculus.questionDrafts";

export function readQuestionDrafts(): QuestionDrafts {
  try {
    const raw: unknown = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    if (!raw || typeof raw !== "object" || Array.isArray(raw)) return {};
    const result: QuestionDrafts = {};
    for (const [thread, requests] of Object.entries(raw)) {
      if (!/^\d+$/.test(thread) || !requests || typeof requests !== "object" || Array.isArray(requests)) continue;
      for (const [request, questions] of Object.entries(requests)) {
        if (!/^\d+$/.test(request) || !questions || typeof questions !== "object" || Array.isArray(questions)) continue;
        for (const [id, value] of Object.entries(questions)) {
          if (!value || typeof value !== "object") continue;
          const d = value as Partial<QuestionDraft>;
          if (!Array.isArray(d.selected) || !d.selected.every((s) => typeof s === "string") || typeof d.text !== "string" || typeof d.dismissed !== "boolean") continue;
          const requests = result[Number(thread)] ??= {};
          requests[Number(request)] = { ...requests[Number(request)], [id]: { selected: d.selected, text: d.text, dismissed: d.dismissed } };
        }
      }
    }
    return result;
  } catch { return {}; }
}

export function saveQuestionDrafts(drafts: QuestionDrafts): QuestionDrafts {
  try { localStorage.setItem(KEY, JSON.stringify(drafts)); } catch { /* keep the in-memory draft */ }
  return drafts;
}

export function pruneQuestionDrafts(drafts: QuestionDrafts, thread: number, keep: (request: number) => boolean): QuestionDrafts {
  const result = { ...drafts };
  const requests = Object.fromEntries(Object.entries(result[thread] ?? {}).filter(([id]) => keep(Number(id))));
  if (Object.keys(requests).length) result[thread] = requests;
  else delete result[thread];
  return saveQuestionDrafts(result);
}
