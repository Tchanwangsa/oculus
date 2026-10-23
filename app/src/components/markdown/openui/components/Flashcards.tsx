import { defineComponent } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { ArrowCounterClockwise, Check, X } from "@phosphor-icons/react";
import { Frame, Pill, Rich } from "./Frame";
import { useWidgetProgress } from "../progress";

export const Card = defineComponent({
  name: "Card",
  description: "One flashcard: a prompt on the front, the answer on the back. Only inside Flashcards.",
  props: z.object({
    front: z.string().describe("The prompt; markdown and $…$ maths allowed"),
    back: z.string().describe("The answer; markdown and $…$ maths allowed"),
  }),
  // Never drawn alone: Flashcards reads its props.
  component: () => null,
});

type CardProps = { front?: unknown; back?: unknown };

/**
 * A deck the student works through: flip a card, then mark it known or not.
 * Unknown cards go round again after the first pass, so the deck ends only
 * when every card is known.
 */
export const Flashcards = defineComponent({
  name: "Flashcards",
  description: "A flashcard deck: flip, then mark each card known or not; unknown cards come round again.",
  props: z.object({
    title: z.string(),
    cards: z.array(Card.ref),
  }),
  component: function FlashcardsView({ props }) {
    const cards = (props.cards ?? []).map((c) => (c?.props ?? {}) as CardProps);
    const progress = useWidgetProgress({ queue: null as number[] | null, known: 0, misses: 0, flipped: false });
    const { queue, known, misses, flipped } = progress.value;
    const flip = () => progress.setValue({ ...progress.value, flipped: !flipped });

    // Built lazily so a deck that streams in longer still starts at card one.
    const order = queue ?? cards.map((_, i) => i);
    const current = order[0];
    const card = current === undefined ? undefined : cards[current];
    const total = cards.length;

    const mark = (ok: boolean) => {
      if (current === undefined) return;
      progress.setValue({ flipped: false, known: known + Number(ok), misses: misses + Number(!ok),
        queue: [...order.slice(1), ...(ok ? [] : [current])] });
    };
    const restart = () => {
      progress.setValue({ queue: null, known: 0, misses: 0, flipped: false });
    };

    if (!total) return null;
    return (
      <Frame
        title={typeof props.title === "string" ? props.title : undefined}
        meta={`${known} of ${total} known${misses ? ` · ${misses} missed` : ""}`}
        footer={
          card ? (
            flipped ? (
              <>
                <Pill onClick={() => mark(false)}>
                  <X size={13} /> Still learning
                </Pill>
                <Pill tone="primary" onClick={() => mark(true)}>
                  <Check size={13} /> Got it
                </Pill>
              </>
            ) : (
              <Pill onClick={flip}>Show answer</Pill>
            )
          ) : (
            <Pill onClick={restart}>
              <ArrowCounterClockwise size={13} /> Start again
            </Pill>
          )
        }
      >
        {card ? (
          <button
            type="button"
            onClick={flip}
            aria-pressed={flipped}
            className="flex min-h-28 w-full cursor-pointer flex-col items-center justify-center gap-2 rounded-lg border border-border-subtle bg-surface px-4 py-5 text-center"
          >
            <span className="text-[11px] font-medium tracking-wide text-muted-foreground uppercase">
              {flipped ? "Answer" : "Prompt"}
            </span>
            <Rich text={flipped ? card.back : card.front} className="text-[15px] leading-relaxed text-foreground" />
          </button>
        ) : (
          <p className="py-6 text-center text-sm text-muted-foreground">
            All {total} cards known{misses ? `, after ${misses} ${misses === 1 ? "miss" : "misses"}` : " first time"}.
          </p>
        )}
      </Frame>
    );
  },
});
