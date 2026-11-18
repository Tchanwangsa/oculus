import { Check, Microphone, SpinnerGap, X, ArrowCounterClockwise } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { TranscriptionSection } from "@/components/settings/transcription/TranscriptionSection";
import { VoiceWaveform } from "./VoiceWaveform";
import type { Dictation } from "./useDictation";
import { cn } from "@/lib/utils";

export function DictationMic({ dictation: d }: { dictation: Dictation }) {
  return <Button type="button" variant="ghost" size="icon-xs" aria-label="Dictate message" title="Dictate message" onClick={() => void d.start()}><Microphone /></Button>;
}

/** The recording strip that stands in for the composer's controls; `small`
 *  keeps to the compact composer's 24px row, so the bar keeps its height. */
export function DictationBar({ dictation: d, small = false, className }: { dictation: Dictation; small?: boolean; className?: string }) {
  const recording = d.phase === "recording";
  const label = recording ? "Listening…" : d.phase === "transcribing" ? "Transcribing…" : d.phase === "cleaning" ? "Tidying up…" : "Preparing microphone…";
  const button = small ? "icon-xs" : "icon-sm";
  return <div className={cn("flex items-center gap-2", small ? "min-h-6" : "min-h-8", className)} role="group" aria-label="Voice dictation">
    <Button type="button" variant="secondary" size={button} aria-label="Cancel dictation" onClick={d.cancel}><X /></Button>
    {recording ? <VoiceWaveform analyser={d.analyser} className={small ? "h-5" : undefined} /> : <div className="flex min-w-0 flex-1 items-center justify-center gap-2 text-xs text-muted-foreground"><SpinnerGap className="shrink-0 motion-safe:animate-spin" size={15} /><span className="truncate">{label}</span></div>}
    <span className="sr-only" role="status">{label}</span>
    {recording && <><span className="text-[11px] text-muted-foreground tabular-nums" aria-label="Recording duration">{Math.floor(d.elapsed / 60)}:{String(d.elapsed % 60).padStart(2, "0")}</span><Button type="button" size={button} aria-label="Finish dictation" title="Finish and tidy up" onClick={() => void d.finish()}><Check /></Button></>}
  </div>;
}

export function DictationFeedback({ dictation: d }: { dictation: Dictation }) {
  return <>
    {d.error && <div className="rounded-lg border border-border-subtle bg-surface-raised p-3 text-xs" role="alert">
      <p className="text-destructive" data-selectable>{d.error}</p>
      {d.raw && <p className="mt-2 max-h-32 overflow-y-auto whitespace-pre-wrap text-foreground" data-selectable>{d.raw}</p>}
      <div className="mt-2 flex flex-wrap gap-2">
        {d.canRetry && <Button type="button" variant="outline" size="xs" onClick={() => void d.retry()}><ArrowCounterClockwise />{d.raw ? "Retry cleanup" : "Retry transcription"}</Button>}
        {d.raw && <Button type="button" variant="outline" size="xs" onClick={d.useTranscript}>Use original transcript</Button>}
        <Button type="button" variant="ghost" size="xs" onClick={d.cancel}>Dismiss</Button>
      </div>
    </div>}
    <Dialog open={d.setup} onOpenChange={d.setSetup}>
      <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-xl">
        <DialogHeader><DialogTitle>Set up dictation</DialogTitle><DialogDescription>Choose how Oculus turns your voice into text. Then a model tidies the transcript before adding it to your draft.</DialogDescription></DialogHeader>
        {d.setupError && <p className="text-xs text-muted-foreground" data-selectable>{d.setupError}</p>}
        <TranscriptionSection />
        <p className="text-xs text-muted-foreground">Choose a small, fast model in Settings → Jobs → Dictation cleanup.</p>
        <div className="flex justify-end"><Button type="button" onClick={() => { d.setSetup(false); void d.start(); }}>Start dictating</Button></div>
      </DialogContent>
    </Dialog>
  </>;
}
