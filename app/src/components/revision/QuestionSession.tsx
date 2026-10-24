import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Grade } from "ts-fsrs";
import { CompactMd } from "@/components/markdown/MdComponents";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { SourceLink } from "./SourceLink";
import {
  localMark,
  ratingForScore,
  recordReview,
  type RevisionItem,
} from "@/lib/revision";
interface Mark {
  total: number;
  max: number;
  feedback: string;
  omission?: string;
  points?: { awarded: number; why: string }[];
  judge_model?: string;
}
export function QuestionSession({
  item,
  setId,
  sessionId,
  onDone,
}: {
  item: RevisionItem;
  setId?: number;
  sessionId?: string;
  onDone: () => void;
}) {
  const [answer, setAnswer] = useState("");
  const [revealed, setRevealed] = useState(false);
  const [mark, setMark] = useState<Mark>();
  const [manual, setManual] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  const [started] = useState(Date.now);
  const submit = async () => {
    setBusy(true);
    setError(undefined);
    try {
      const local = localMark(item, answer);
      if (local) {
        setMark({
          total: local.score,
          max: local.max,
          feedback: local.score
            ? "Correct."
            : "Compare your answer with the model answer.",
        });
      } else {
        setMark(
          await invoke<Mark>("revision_mark_answer", {
            itemId: item.id,
            answer,
          }),
        );
      }
      setRevealed(true);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  const selfMax = item.rubric.reduce((sum, p) => sum + p.marks, 0) || 1;
  const finish = async (rating: Grade) => {
    if (busy) return;
    setBusy(true);
    setError(undefined);
    try {
      await recordReview(item, {
        answer,
        rating,
        score:
          mark?.total ??
          (setId != null && item.kind === "card" ? Number(rating > 1) : null),
        max: mark?.max ?? (setId != null && item.kind === "card" ? 1 : null),
        feedback: mark
          ? [mark.feedback, mark.omission].filter(Boolean).join("\n")
          : null,
        judge_model: mark?.judge_model ?? null,
        overridden:
          manual || (!!mark && rating !== ratingForScore(mark.total, mark.max)),
        seconds: (Date.now() - started) / 1000,
        set_id: setId,
        session_id: sessionId,
        scheduled: setId == null,
      });
      onDone();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };
  return (
    <article className="min-w-0 space-y-4 rounded-xl border border-border bg-card p-5">
      <div className="flex flex-wrap gap-2 text-xs text-muted-foreground">
        <span className="rounded-full bg-surface px-2 py-1 capitalize">
          {item.level}
        </span>
        <span className="rounded-full bg-surface px-2 py-1">{item.kind}</span>
        {item.topic.map((t) => (
          <span key={t} className="px-1 py-1">
            {t}
          </span>
        ))}
      </div>
      <CompactMd text={item.prompt} />
      {item.kind === "choice" ? (
        <div className="grid gap-2">
          {item.options.map((choice, i) => (
            <button
              type="button"
              key={i}
              disabled={revealed || busy}
              aria-pressed={answer === choice}
              className={`rounded-xl border px-4 py-3 text-left text-sm ${answer === choice ? "border-brand bg-brand-muted" : "border-border hover:bg-accent"}`}
              onClick={() => setAnswer(choice)}
            >
              <CompactMd text={choice} />
            </button>
          ))}
        </div>
      ) : (
        item.kind !== "card" &&
        (item.kind === "numeric" ? (
          <Input
            aria-label="Your numeric answer"
            inputMode="decimal"
            disabled={revealed || busy}
            value={answer}
            onChange={(e) => setAnswer(e.target.value)}
          />
        ) : (
          <Textarea
            aria-label="Your answer"
            placeholder="Explain your reasoning…"
            className="min-h-32"
            disabled={revealed || busy}
            value={answer}
            onChange={(e) => setAnswer(e.target.value)}
          />
        ))
      )}
      {!revealed &&
        (item.kind === "card" ? (
          <Button onClick={() => setRevealed(true)}>Show answer</Button>
        ) : (
          <Button
            disabled={busy || !answer.trim()}
            onClick={() => void submit()}
          >
            {busy ? "Marking…" : "Submit answer"}
          </Button>
        ))}
      {revealed && (
        <div className="space-y-3 border-t border-border-subtle pt-4">
          <h3 className="text-sm font-semibold">Model answer</h3>
          <CompactMd text={item.answer} />
          {manual && (
            <label className="block text-sm">
              Your mark (out of {selfMax})
              <Input
                type="number"
                min={0}
                max={selfMax}
                step="any"
                value={mark?.total ?? 0}
                onChange={(e) => {
                  const total = Number(e.target.value);
                  if (Number.isFinite(total) && total >= 0 && total <= selfMax)
                    setMark({ total, max: selfMax, feedback: "Self marked" });
                }}
              />
            </label>
          )}
          {mark && (
            <>
              <p className="text-sm font-semibold">
                {mark.total} / {mark.max}
              </p>
              <p className="text-sm">{mark.feedback}</p>
              {mark.points?.map((point, i) => (
                <p key={i} className="text-xs text-muted-foreground">
                  {point.awarded} · {point.why}
                </p>
              ))}
              {mark.omission && (
                <p className="text-sm text-muted-foreground">{mark.omission}</p>
              )}
            </>
          )}
          <p className="text-xs text-muted-foreground">
            {setId == null
              ? "Choose how well you recalled it. This schedules your next review."
              : "Record your result. Practice quizzes leave the review schedule unchanged."}
          </p>
          <div className="flex flex-wrap gap-2">
            {(["Again", "Hard", "Good", "Easy"] as const).map((label, i) => (
              <Button
                key={label}
                size="sm"
                disabled={busy}
                variant={
                  mark && ratingForScore(mark.total, mark.max) === i + 1
                    ? "default"
                    : "outline"
                }
                onClick={() => void finish((i + 1) as Grade)}
              >
                {label}
              </Button>
            ))}
          </div>
        </div>
      )}
      {error && (
        <div className="space-y-2">
          <p role="alert" className="text-sm text-destructive">
            {error}
          </p>
          {!revealed && (
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                setManual(true);
                setMark({ total: 0, max: selfMax, feedback: "Self marked" });
                setRevealed(true);
              }}
            >
              Mark it myself
            </Button>
          )}
        </div>
      )}
      <SourceLink source={item.style_ref} />
    </article>
  );
}
