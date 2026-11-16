import type { Ref } from "react";
import { MentionInput, type MentionInputHandle } from "@/components/harness/mentions/MentionInput";
import type { QuestionComposer } from "./useQuestionComposer";

/**
 * The composer's text while a question waits: the current question's typed
 * answer in an editor of its own (`flow.editorKey`), so the message draft is
 * untouched underneath, and Enter answers. A choice-only question has
 * nothing to type, so the box only says to choose above, and digit keys
 * reach the panel.
 */
export function QuestionEditor({ flow, inputRef }: { flow: QuestionComposer; inputRef: Ref<MentionInputHandle> }) {
  if (!flow.question.allowsTypedAnswer) {
    return (
      <div data-question-choice-only className="min-h-[16px] select-none text-[13px] leading-[16px] text-muted-foreground">
        {flow.placeholder}
      </div>
    );
  }
  return (
    <MentionInput
      key={flow.editorKey}
      ref={inputRef}
      initialText={flow.draft.text}
      skills={null}
      onEdit={(next) => flow.type(next)}
      onKeyDown={flow.keyDown}
      placeholder={flow.placeholder}
    />
  );
}
