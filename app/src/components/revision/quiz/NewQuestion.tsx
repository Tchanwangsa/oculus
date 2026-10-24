import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { PillTabs } from "@/components/ui/table/PillTabs";
import { createItem, type RevisionItem } from "@/lib/revision";

type Kind = Extract<RevisionItem["kind"], "card" | "choice" | "numeric" | "short">;

const KINDS: { value: Kind; label: string }[] = [
  { value: "card", label: "Card" },
  { value: "choice", label: "Choice" },
  { value: "numeric", label: "Numeric" },
  { value: "short", label: "Written" },
];

/**
 * A question the student writes by hand. It is active at once, unlike an
 * agent's draft, so it can go straight into a quiz or Review. Rubric-marked
 * structured questions stay with the agent and the question editor.
 */
export function NewQuestion({
  subjectId,
  topicId,
  onDone,
}: {
  subjectId: number;
  /** The selected topic, which the question joins. */
  topicId?: number;
  onDone: () => void;
}) {
  const [kind, setKind] = useState<Kind>("card");
  const [prompt, setPrompt] = useState("");
  const [answer, setAnswer] = useState("");
  const [options, setOptions] = useState("");
  const [level, setLevel] = useState<RevisionItem["level"]>("medium");
  const [error, setError] = useState<string>();

  const save = async () => {
    try {
      await createItem({
        kind,
        prompt: prompt.trim(),
        answer: answer.trim(),
        level,
        subject_id: subjectId,
        status: "active",
        created_by: "user",
        topic_ids: topicId ? [topicId] : undefined,
        ...(kind === "choice"
          ? { options: options.split("\n").map((o) => o.trim()).filter(Boolean) }
          : {}),
        ...(kind === "numeric" ? { tolerance: 0 } : {}),
      });
      onDone();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div className="space-y-3 rounded-xl border border-ring/50 bg-card p-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <PillTabs tabs={KINDS} value={kind} onChange={setKind} />
        <PillTabs
          tabs={[
            { value: "easy", label: "Easy" },
            { value: "medium", label: "Medium" },
            { value: "hard", label: "Hard" },
          ]}
          value={level}
          onChange={setLevel}
        />
      </div>
      <Textarea
        autoFocus
        aria-label="Question"
        placeholder={kind === "card" ? "Front" : "Question"}
        value={prompt}
        onChange={(e) => setPrompt(e.target.value)}
      />
      {kind === "choice" && (
        <Textarea
          aria-label="Options, one per line"
          placeholder="Options, one per line"
          value={options}
          onChange={(e) => setOptions(e.target.value)}
        />
      )}
      {kind === "short" ? (
        <Textarea
          aria-label="Model answer"
          placeholder="Model answer"
          value={answer}
          onChange={(e) => setAnswer(e.target.value)}
        />
      ) : (
        <Input
          aria-label={kind === "card" ? "Back" : "Answer"}
          placeholder={
            kind === "card"
              ? "Back"
              : kind === "choice"
                ? "The correct option, exactly as written"
                : "The exact number"
          }
          inputMode={kind === "numeric" ? "decimal" : undefined}
          value={answer}
          onChange={(e) => setAnswer(e.target.value)}
        />
      )}
      {error && (
        <p role="alert" className="text-xs text-destructive">
          {error}
        </p>
      )}
      <div className="flex gap-2">
        <Button size="sm" disabled={!prompt.trim() || !answer.trim()} onClick={() => void save()}>
          Add question
        </Button>
        <Button size="sm" variant="ghost" onClick={onDone}>
          Cancel
        </Button>
      </div>
    </div>
  );
}
