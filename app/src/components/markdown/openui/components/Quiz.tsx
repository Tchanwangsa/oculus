import { SaveQuestions } from "@/components/revision/SaveWidget";
import { defineComponent } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { CheckCircle, XCircle } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Frame, Pill, Rich } from "./Frame";
import { useWidgetProgress } from "../progress";

export const Option = defineComponent({
  name: "Option",
  description: "One answer choice; mark exactly one option per question correct = true. Only inside Question.",
  props: z.object({
    text: z.string(),
    correct: z.boolean().optional(),
  }),
  component: () => null,
});

export const Question = defineComponent({
  name: "Question",
  description: "A multiple-choice question. Only inside Quiz.",
  props: z.object({
    prompt: z.string().describe("Markdown and $…$ maths allowed"),
    options: z.array(Option.ref),
    explanation: z.string().optional().describe("Shown after checking"),
  }),
  component: () => null,
});

type OptionProps = { text?: unknown; correct?: unknown };
type QuestionProps = { prompt?: unknown; options?: { props?: OptionProps }[]; explanation?: unknown };

/** Multiple choice, one question at a time: pick, check, see why, move on. */
export const Quiz = defineComponent({
  name: "Quiz",
  description: "A multiple-choice quiz, one question at a time, scored as it goes.",
  props: z.object({
    title: z.string(),
    questions: z.array(Question.ref),
  }),
  component: function QuizView({ props }) {
    const questions = (props.questions ?? []).map((q) => (q?.props ?? {}) as QuestionProps);
    const progress = useWidgetProgress({ at: 0, picked: null as number | null, checked: false, score: 0 });
    const { at, picked, checked, score } = progress.value;

    const total = questions.length;
    if (!total) return null;
    const done = at >= total;
    const q = questions[Math.min(at, total - 1)];
    const options = (q.options ?? []).map((o) => (o?.props ?? {}) as OptionProps);
    const right = (i: number) => options[i]?.correct === true;

    const check = () => {
      if (picked === null || checked) return;
      progress.setValue({ ...progress.value, checked: true, score: score + Number(right(picked)) });
    };
    const next = () => {
      progress.setValue({ ...progress.value, at: at + 1, picked: null, checked: false });
    };
    const restart = () => {
      progress.setValue({ at: 0, picked: null, checked: false, score: 0 });
    };

    return (
      <Frame
        title={typeof props.title === "string" ? props.title : undefined}
        meta={done ? `${score} / ${total}` : `${at + 1} of ${total} · ${score} right`}
        footer={<>
          <SaveQuestions items={questions.map(q=>{const options=(q.options??[]).map(o=>o.props??{});return {kind:"choice",prompt:String(q.prompt??""),answer:String(options.find(o=>o.correct===true)?.text??""),options:options.map(o=>String(o.text??""))};})}/>
          {done ? (
            <Pill onClick={restart}>Try again</Pill>
          ) : checked ? (
            <Pill tone="primary" onClick={next}>
              {at + 1 < total ? "Next question" : "See score"}
            </Pill>
          ) : (
            <Pill tone="primary" onClick={check} disabled={picked === null}>
              Check
            </Pill>
          )
        }</>}
      >
        {done ? (
          <p className="py-4 text-center text-sm text-foreground">
            You got <span className="tabular-nums font-semibold">{score}</span> of{" "}
            <span className="tabular-nums">{total}</span>.
          </p>
        ) : (
          <>
            <Rich text={q.prompt} className="block text-sm leading-relaxed text-foreground" />
            <div role="group" aria-label="Answer choices" className="mt-2 flex flex-col gap-1.5">
              {options.map((o, i) => {
                const mine = picked === i;
                const good = checked && right(i);
                const bad = checked && mine && !right(i);
                return (
                  <button
                    key={i}
                    type="button"
                    aria-pressed={mine}
                    disabled={checked}
                    onClick={() => progress.setValue({ ...progress.value, picked: i })}
                    className={cn(
                      "flex cursor-pointer items-center gap-2 rounded-lg border px-3 py-2 text-left text-sm transition-[color,background-color,border-color] disabled:cursor-default",
                      !checked && (mine ? "border-brand bg-brand-muted" : "border-border hover:bg-accent"),
                      good && "border-success bg-success/10",
                      bad && "border-destructive bg-destructive/10",
                      checked && !good && !bad && "border-border opacity-70",
                    )}
                  >
                    <Rich text={o.text} className="min-w-0 flex-1" />
                    {good && <CheckCircle size={16} weight="fill" className="text-success" />}
                    {bad && <XCircle size={16} weight="fill" className="text-destructive" />}
                  </button>
                );
              })}
            </div>
            {checked && typeof q.explanation === "string" && (
              <Rich text={q.explanation} className="mt-2 block text-[13px] leading-relaxed text-muted-foreground" />
            )}
          </>
        )}
      </Frame>
    );
  },
});
