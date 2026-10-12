import { useState } from "react";
import { Play, Stop } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { displayCode, humanizeSlug } from "@/lib/format/format";
import type { Pronunciation, PronunciationDecision } from "@/lib/studio/pronunciations";
import type { useClipPlayer } from "../useClipPlayer";
import { SentenceWithWord } from "./SentenceWithWord";

type Which = "original" | "suggestion";

/**
 * One suggested reading: the word and the reading, its sentence with the
 * word picked out, where it is said, the two samples, and the three
 * verdicts. A row sent back as wrong keeps its samples and shows the
 * student's note instead of the verdicts, until the agent suggests again.
 * Buttons wait while a request runs; a failure shows on the row.
 */
export function PronunciationRow({ item, player, onPlay, onDecide, onWrong }: {
  item: Pronunciation;
  player: ReturnType<typeof useClipPlayer>;
  onPlay: (which: Which) => void;
  onDecide: (decision: PronunciationDecision) => Promise<void>;
  onWrong: (note: string | null) => Promise<void>;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [asking, setAsking] = useState(false);
  const [note, setNote] = useState("");
  const wrong = item.status === "wrong";
  const sampleError = player.error?.key.startsWith(`${item.id}:`) ? player.error.message : null;

  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (cause) {
      console.error("Pronunciation update failed", cause);
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  };

  const send = () =>
    run(async () => {
      await onWrong(note.trim() || null);
      setAsking(false);
      setNote("");
    });

  const sample = (which: Which, label: string) => {
    const phase = player.phaseOf(`${item.id}:${which}`);
    return (
      <Button
        variant="outline"
        size="xs"
        className="w-[6.75rem]"
        disabled={busy}
        onClick={() => (phase === "idle" ? onPlay(which) : player.stop())}
      >
        {phase === "playing" ? <Stop size={12} weight="fill" /> : <Play size={12} weight="fill" />}
        {phase === "making" ? "Making…" : label}
      </Button>
    );
  };

  return (
    <li className="py-3">
      <div className="flex items-start justify-between gap-4">
        <div className={cn("min-w-0", wrong && "text-muted-foreground")}>
          <p className="text-[13px]">
            <span className={cn("font-medium", !wrong && "text-foreground")}>{item.word}</span>
            <span className="text-muted-foreground"> read as </span>
            <span className={cn(!wrong && "text-foreground")}>{item.reading}</span>
          </p>
          <p className="mt-0.5 text-xs text-muted-foreground">
            <SentenceWithWord sentence={item.sentence} word={item.word} strong={!wrong} />
          </p>
          <p className="mt-0.5 text-[11px] text-muted-foreground tabular-nums">
            {item.subject ? displayCode(item.subject) : "Library"} · {humanizeSlug(item.video)} · scene {item.scene}
          </p>
        </div>
        <div className="flex shrink-0 gap-1.5">
          {sample("original", "Original")}
          {sample("suggestion", "Suggestion")}
        </div>
      </div>

      {wrong ? (
        <p className="mt-2 text-[11px] leading-relaxed text-muted-foreground">
          Sent back to the agent{item.note ? <>: “{item.note}”</> : ", with no note"}
        </p>
      ) : asking ? (
        <div className="mt-2.5">
          <p className="text-[11px] text-muted-foreground">How is it said? (optional)</p>
          <div className="mt-1 flex items-center gap-1.5">
            <Input
              autoFocus
              value={note}
              disabled={busy}
              placeholder="NESS-ter-off, stress on the first part"
              onChange={(e) => setNote(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void send();
                if (e.key === "Escape") setAsking(false);
              }}
            />
            <Button size="xs" disabled={busy} onClick={() => void send()}>
              Send
            </Button>
            <Button variant="ghost" size="xs" disabled={busy} onClick={() => setAsking(false)}>
              Cancel
            </Button>
          </div>
        </div>
      ) : (
        <div className="mt-2.5 flex flex-wrap gap-1.5">
          <Button variant="outline" size="xs" disabled={busy} onClick={() => void run(() => onDecide("correct"))}>
            Suggestion correct
          </Button>
          <Button variant="outline" size="xs" disabled={busy} onClick={() => setAsking(true)}>
            Suggestion wrong
          </Button>
          <Button variant="outline" size="xs" disabled={busy} onClick={() => void run(() => onDecide("fine"))}>
            Pronunciation is fine
          </Button>
        </div>
      )}

      {error || sampleError ? (
        <p className="mt-2 text-[11px] leading-relaxed text-destructive" data-selectable>
          {error ?? sampleError}
        </p>
      ) : null}
    </li>
  );
}
