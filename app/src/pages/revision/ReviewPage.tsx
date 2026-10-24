import { useEffect, useState } from "react";
import { RevisionPage } from "@/components/revision/RevisionPage";
import { QuestionSession } from "@/components/revision/QuestionSession";
import { useRevisionClock } from "@/hooks/revision/useRevisionClock";
import { useRevisionData } from "@/layouts/RevisionLayout";

export default function ReviewPage() {
  const { items, scope, topicScope } = useRevisionData();
  const now = useRevisionClock();
  const [pending, setPending] = useState<Map<number, number>>(new Map());
  const [reviewed, setReviewed] = useState(0);

  useEffect(() => {
    setPending(new Map());
    setReviewed(0);
  }, [scope, topicScope]);

  // Hide only the revision just answered until the database reload catches up.
  // A learning card's next revision can rejoin the queue when it becomes due.
  useEffect(() => {
    setPending((previous) => {
      const next = new Map(
        [...previous].filter(([id, reps]) =>
          items.some((item) => item.id === id && item.reps <= reps),
        ),
      );
      return next.size === previous.size ? previous : next;
    });
  }, [items]);

  const due = items
    .filter(
      (item) =>
        item.status === "active" &&
        new Date(item.due).getTime() <= now &&
        pending.get(item.id) !== item.reps,
    )
    .sort((a, b) => a.due.localeCompare(b.due));
  const current = due[0];

  return (
    <RevisionPage title="Review">
      <p className="text-sm text-muted-foreground">
        {due.length} due · {reviewed} reviewed this session
      </p>
      {current ? (
        <QuestionSession
          key={`${current.id}:${current.reps}`}
          item={current}
          onDone={() => {
            setPending((previous) =>
              new Map(previous).set(current.id, current.reps),
            );
            setReviewed((count) => count + 1);
          }}
        />
      ) : (
        <p className="py-12 text-center text-sm text-muted-foreground">
          You’re up to date. Accept draft questions in Quizzes to begin
          reviewing.
        </p>
      )}
    </RevisionPage>
  );
}
