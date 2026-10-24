import { useEffect, useState } from "react";
import { Plus } from "@phosphor-icons/react";
import { CompactMd } from "@/components/markdown/MdComponents";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { RevisionPage } from "@/components/revision/RevisionPage";
import { ItemEditor } from "@/components/revision/ItemEditor";
import { TopicPicker } from "@/components/revision/TopicPicker";
import { SourceLink } from "@/components/revision/SourceLink";
import { QuestionSession } from "@/components/revision/QuestionSession";
import { NewQuestion } from "@/components/revision/quiz/NewQuestion";
import { useRevisionData } from "@/layouts/RevisionLayout";
import {
  updateItem,
  updateSet,
  deleteItem,
  createQuiz,
  addSetItem,
  getSetItems,
  getReviews,
  type RevisionItem,
  type RevisionReview,
} from "@/lib/revision";
export default function QuizzesPage() {
  const { items, sets, scope, topicScope, creationTopic } = useRevisionData();
  const [editing, setEditing] = useState<number>();
  const [writing, setWriting] = useState(false);
  const [title, setTitle] = useState("");
  const [picked, setPicked] = useState<Set<number>>(new Set());
  const [error, setError] = useState<string>();
  const [quiz, setQuiz] = useState<{
    id: number;
    items: RevisionItem[];
    at: number;
    session: string;
  }>();
  const [history, setHistory] = useState<RevisionReview[]>([]);
  useEffect(() => {
    setPicked(new Set());
    setQuiz(undefined);
  }, [scope, topicScope]);
  useEffect(() => {
    setPicked((previous) => {
      const active = new Set(
        items.filter((item) => item.status === "active").map((item) => item.id),
      );
      const next = new Set([...previous].filter((id) => active.has(id)));
      return next.size === previous.size ? previous : next;
    });
  }, [items]);
  const selected = items
    .filter((item) => item.status === "active" && picked.has(item.id))
    .map((item) => item.id);
  const run = (fn: () => Promise<unknown>) => {
    setError(undefined);
    void fn().catch((e) => setError(e instanceof Error ? e.message : String(e)));
  };
  // Revision is scoped to one subject; "general" means none is synced yet.
  const subjectId = creationTopic?.subject_id ?? (typeof scope === "number" ? scope : null);
  useEffect(() => {
    void getReviews()
      .then(setHistory)
      .catch((e) => setError(String(e)));
  }, [quiz]);
  const make = async () => {
    if (!title.trim() || subjectId == null) return;
    await createQuiz(
      title.trim(),
      subjectId,
      selected,
      creationTopic ? [creationTopic.id] : undefined,
    );
    setTitle("");
    setPicked(new Set());
  };
  if (quiz) {
    const item = quiz.items[quiz.at];
    return (
      <RevisionPage
        title={sets.find((s) => s.id === quiz.id)?.title ?? "Quiz"}
        actions={
          <Button variant="ghost" size="sm" onClick={() => setQuiz(undefined)}>
            Close quiz
          </Button>
        }
      >
        <p className="text-xs text-muted-foreground">
          {Math.min(quiz.at + 1, quiz.items.length)} of {quiz.items.length}
        </p>
        {item ? (
          <QuestionSession
            key={`${quiz.session}:${item.id}`}
            item={item}
            setId={quiz.id}
            sessionId={quiz.session}
            onDone={() => setQuiz({ ...quiz, at: quiz.at + 1 })}
          />
        ) : (
          <p className="py-10 text-center text-sm">
            Quiz complete. Your results are saved.
          </p>
        )}
      </RevisionPage>
    );
  }
  const drafts = items.filter((i) => i.status === "draft");
  return (
    <RevisionPage title="Quizzes">
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      <section className="space-y-3">
        <h2 className="text-base font-semibold">Your quizzes</h2>
        {sets.map((set) => {
          const reviews = history.filter((r) => r.set_id === set.id);
          const sessions = [
            ...new Set(reviews.map((r) => r.session_id).filter(Boolean)),
          ];
          return (
            <div
              key={set.id}
              className="flex flex-wrap items-center gap-3 border-b border-border-subtle py-3"
            >
              <Input
                aria-label="Quiz title"
                className="min-w-32 flex-1"
                defaultValue={set.title}
                key={set.title}
                onBlur={(e) => {
                  if (e.target.value.trim() && e.target.value !== set.title)
                    run(() =>
                      updateSet(set.id, { title: e.target.value.trim() }),
                    );
                }}
              />
              <TopicPicker
                subjectId={set.subject_id}
                value={set.topic_ids}
                onChange={(topic_ids) =>
                  run(() => updateSet(set.id, { topic_ids }))
                }
              />
              <span className="text-xs text-muted-foreground">
                {sessions.length}{" "}
                {sessions.length === 1 ? "attempt" : "attempts"}
                {sessions[0]
                  ? ` · last ${reviews.filter((r) => r.session_id === sessions[0]).reduce((s, r) => s + (r.score ?? 0), 0)} / ${reviews.filter((r) => r.session_id === sessions[0]).reduce((s, r) => s + (r.max ?? 0), 0)}`
                  : ""}
              </span>
              {selected.length > 0 && (
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() =>
                    run(async () => {
                      for (const id of selected) await addSetItem(set.id, id);
                      setPicked(new Set());
                    })
                  }
                >
                  Add {selected.length} ticked
                </Button>
              )}
              <Button
                size="sm"
                variant="outline"
                onClick={() =>
                  run(async () => {
                    const rows = (await getSetItems(set.id)).filter(
                      (i) => i.status === "active",
                    );
                    if (!rows.length)
                      throw new Error(
                        `“${set.title}” has no accepted questions yet. Tick some below and add them.`,
                      );
                    setQuiz({
                      id: set.id,
                      items: rows,
                      at: 0,
                      session: crypto.randomUUID(),
                    });
                  })
                }
              >
                Take quiz
              </Button>
              {sessions.length > 0 && (
                <details className="basis-full text-xs text-muted-foreground">
                  <summary className="cursor-pointer py-1">
                    Score history
                  </summary>
                  <div className="space-y-2 py-2">
                    {sessions.map((session) => {
                      const attempts = reviews.filter(
                        (r) => r.session_id === session,
                      );
                      return (
                        <div
                          key={session}
                          className="flex flex-wrap justify-between gap-2"
                        >
                          <span>
                            {new Date(attempts[0].at).toLocaleString()} ·{" "}
                            {attempts.length} answered
                          </span>
                          <span className="tabular-nums">
                            {attempts.reduce(
                              (sum, r) => sum + (r.score ?? 0),
                              0,
                            )}{" "}
                            /{" "}
                            {attempts.reduce((sum, r) => sum + (r.max ?? 0), 0)}
                          </span>
                        </div>
                      );
                    })}
                  </div>
                </details>
              )}
            </div>
          );
        })}
        {!sets.length && (
          <p className="text-sm text-muted-foreground">
            Name a quiz to create it. Tick accepted questions below to put
            them in it, now or later.
          </p>
        )}
        <div className="flex flex-wrap gap-2">
          <Input
            aria-label="New quiz title"
            placeholder="Quiz title"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            className="min-w-40 flex-1"
          />
          <Button
            size="sm"
            disabled={!title.trim() || subjectId == null}
            onClick={() => run(make)}
          >
            {selected.length
              ? `Create quiz with ${selected.length}`
              : "Create quiz"}
          </Button>
        </div>
      </section>
      <section className="space-y-3">
        <div className="flex items-center justify-between gap-2">
          <h2 className="text-base font-semibold">
            Questions{drafts.length ? ` · ${drafts.length} drafts` : ""}
          </h2>
          {!writing && subjectId != null && (
            <Button size="sm" variant="outline" onClick={() => setWriting(true)}>
              <Plus />
              New question
            </Button>
          )}
        </div>
        {writing && subjectId != null && (
          <NewQuestion
            subjectId={subjectId}
            topicId={creationTopic?.id}
            onDone={() => setWriting(false)}
          />
        )}
        {!items.length && !writing && (
          <p className="text-sm text-muted-foreground">
            Write one with New question, ask chat to draft some from your
            course, or save a Quiz or Flashcards widget.
          </p>
        )}
        {items.map((item) => (
          <article
            key={item.id}
            className="space-y-3 rounded-xl border border-border p-4"
          >
            <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
              {item.status === "active" && (
                <input
                  type="checkbox"
                  aria-label={`Include question ${item.id} in quiz`}
                  checked={picked.has(item.id)}
                  onChange={(e) =>
                    setPicked((s) => {
                      const next = new Set(s);
                      if (e.target.checked) next.add(item.id);
                      else next.delete(item.id);
                      return next;
                    })
                  }
                />
              )}
              <span className="capitalize">
                {item.level} · {item.kind} · {item.status}
              </span>
              <div className="ml-auto flex gap-1">
                <Button
                  size="xs"
                  variant="ghost"
                  onClick={() =>
                    setEditing(editing === item.id ? undefined : item.id)
                  }
                >
                  Edit
                </Button>
                {item.status !== "active" ? (
                  <Button
                    size="xs"
                    variant="outline"
                    onClick={() =>
                      run(() => updateItem(item.id, { status: "active" }))
                    }
                  >
                    Accept
                  </Button>
                ) : (
                  <Button
                    size="xs"
                    variant="ghost"
                    onClick={() =>
                      run(() => updateItem(item.id, { status: "suspended" }))
                    }
                  >
                    Suspend
                  </Button>
                )}
                <Button
                  size="xs"
                  variant="ghost"
                  onClick={() => run(() => deleteItem(item.id))}
                >
                  Delete
                </Button>
              </div>
            </div>
            <CompactMd text={item.prompt} />
            <div className="flex flex-wrap items-center gap-2">
              <TopicPicker
                subjectId={item.subject_id}
                value={item.topic_ids}
                onChange={(topic_ids) =>
                  run(() => updateItem(item.id, { topic_ids }))
                }
              />
              <SourceLink source={item.style_ref} />
            </div>
            {editing === item.id && (
              <ItemEditor item={item} onClose={() => setEditing(undefined)} />
            )}
          </article>
        ))}
      </section>
    </RevisionPage>
  );
}
