import { useCallback, useEffect, useRef, useState } from "react";
import { Section } from "@/components/settings/shared/section";
import { displayCode, humanizeSlug } from "@/lib/format/format";
import {
  decidePronunciation,
  markPronunciationWrong,
  pronunciations,
  pronunciationSample,
  type Pronunciation,
  type PronunciationDecision,
} from "@/lib/studio/pronunciations";
import { useClipPlayer } from "../useClipPlayer";
import { PronunciationRow } from "./PronunciationRow";

/** What a decision wrote, said in the decided row's place. */
function decidedLine(item: Pronunciation, decision: PronunciationDecision): string {
  const lexicon = item.subject ? `${displayCode(item.subject)}'s lexicon` : "the library's lexicon";
  const what = decision === "correct" ? `Added “${item.reading}” to ${lexicon}` : `Kept the guess in ${lexicon}`;
  return `${item.word}: ${what} — applies when ${humanizeSlug(item.video)} is next narrated`;
}

/**
 * Settings → Studio → Pronunciation's review of the readings the Studio agent suggested for
 * words Kokoro guessed (`app/src/lib/studio/pronunciations.ts`). The CLI
 * adds items while the page is open, so the list reloads on mount and
 * whenever the window regains focus. One clip plays at a time across the
 * section. A decided row gives way to a line saying where it went, until
 * the next reload.
 */
export function PronunciationsSection() {
  const [items, setItems] = useState<Pronunciation[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [decided, setDecided] = useState<Record<string, string>>({});
  const player = useClipPlayer();
  const loads = useRef(0);

  const load = useCallback(async () => {
    const mine = ++loads.current;
    try {
      const read = await pronunciations();
      if (loads.current !== mine) return;
      setItems(read);
      setDecided({});
      setLoadError(null);
    } catch (cause) {
      console.error("Pronunciations read failed", cause);
      if (loads.current === mine) setLoadError(String(cause));
    }
  }, []);

  useEffect(() => {
    void load();
    const onFocus = () => void load();
    window.addEventListener("focus", onFocus);
    return () => {
      window.removeEventListener("focus", onFocus);
      loads.current += 1;
    };
  }, [load]);

  const decide = async (item: Pronunciation, decision: PronunciationDecision) => {
    await decidePronunciation(item.id, decision);
    if (player.current?.key.startsWith(`${item.id}:`)) player.stop();
    setDecided((d) => ({ ...d, [item.id]: decidedLine(item, decision) }));
  };

  const markWrong = async (item: Pronunciation, note: string | null) => {
    const updated = await markPronunciationWrong(item.id, note);
    setItems((list) => list?.map((i) => (i.id === updated.id ? updated : i)) ?? list);
  };

  return (
    <Section
      title="Pronunciations"
      description="Review readings before the next narration."
    >
      {loadError ? (
        <p className="py-2 text-[11px] leading-relaxed text-destructive" data-selectable>
          {loadError}
        </p>
      ) : items === null ? null : items.length === 0 ? (
        <p className="py-2 text-xs text-muted-foreground">No words waiting for review.</p>
      ) : (
        <ul className="divide-y divide-border">
          {items.map((item) =>
            decided[item.id] ? (
              <li key={item.id} className="py-3 text-xs text-muted-foreground">
                {decided[item.id]}
              </li>
            ) : (
              <PronunciationRow
                key={item.id}
                item={item}
                player={player}
                onPlay={(which) => void player.play(`${item.id}:${which}`, () => pronunciationSample(item.id, which))}
                onDecide={(decision) => decide(item, decision)}
                onWrong={(note) => markWrong(item, note)}
              />
            ),
          )}
        </ul>
      )}
    </Section>
  );
}
