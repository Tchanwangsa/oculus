import { defineComponent } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { Frame, Pill, Rich } from "./Frame";
import { useWidgetProgress } from "../progress";

export const Step = defineComponent({
  name: "Step",
  description: "One step of a worked solution. Only inside Steps.",
  props: z.object({
    title: z.string().describe("What this step does, e.g. \"Subtract 3 from both sides\""),
    work: z.string().optional().describe("The working; markdown and $…$ maths allowed"),
  }),
  component: () => null,
});

type StepProps = { title?: unknown; work?: unknown };

/** A worked problem revealed one step at a time, so the student can try the
 *  next move before seeing it. */
export const Steps = defineComponent({
  name: "Steps",
  description: "A worked problem revealed one step at a time; answer is shown after the last step.",
  props: z.object({
    problem: z.string().describe("The problem statement; markdown and $…$ maths allowed"),
    steps: z.array(Step.ref),
    answer: z.string().optional(),
  }),
  component: function StepsView({ props }) {
    const steps = (props.steps ?? []).map((s) => (s?.props ?? {}) as StepProps);
    const progress = useWidgetProgress(0);
    const shown = progress.value;
    const setShown = progress.setValue;
    const total = steps.length;
    if (!total) return null;
    const all = shown >= total;
    return (
      <Frame
        title={typeof props.problem === "string" ? props.problem : undefined}
        meta={`${Math.min(shown, total)} of ${total} steps`}
        footer={
          all ? (
            <Pill onClick={() => setShown(0)}>Hide steps</Pill>
          ) : (
            <>
              <Pill tone="primary" onClick={() => setShown(shown + 1)}>
                {shown === 0 ? "Show first step" : "Next step"}
              </Pill>
              <Pill tone="ghost" onClick={() => setShown(total)}>
                Show all
              </Pill>
            </>
          )
        }
      >
        <ol className="flex flex-col gap-2">
          {steps.slice(0, shown).map((s, i) => (
            <li key={i} className="flex gap-2.5">
              <span className="mt-0.5 flex size-5 shrink-0 items-center justify-center rounded-full bg-brand-muted text-[11px] font-semibold tabular-nums text-brand">
                {i + 1}
              </span>
              <div className="min-w-0 flex-1">
                <Rich text={s.title} className="block text-sm font-medium text-foreground" />
                <Rich text={s.work} className="block text-sm leading-relaxed text-foreground/90" />
              </div>
            </li>
          ))}
        </ol>
        {all && typeof props.answer === "string" && (
          <div className="mt-3 rounded-lg bg-surface px-3 py-2 text-sm">
            <span className="mr-1.5 text-muted-foreground">Answer</span>
            <Rich text={props.answer} className="font-medium text-foreground" />
          </div>
        )}
      </Frame>
    );
  },
});
