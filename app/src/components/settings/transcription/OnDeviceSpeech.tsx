import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { appleLocale, localeName, type AppleSpeechStatus } from "@/lib/lectures/transcribe";

/** `undefined` is "not answered yet"; `null` is a status call that failed. */
export type SpeechStatus = AppleSpeechStatus | null | undefined;

/** The engine row's one line: the locale a run would use, or why there is none. */
export function speechStatus(status: SpeechStatus, language: string | null): { text: string; ready: boolean } {
  if (status === undefined) return { text: "", ready: false };
  if (status === null) return { text: "Could not ask macOS", ready: false };
  if (!status.available) return { text: "Not available on this Mac", ready: false };
  const locale = appleLocale(language, status.defaultLocale);
  return { text: locale ? localeName(locale) : "English", ready: true };
}

/**
 * What on-device speech is, whether this Mac has it, and which languages are
 * already downloaded. Nothing to set: its switch and place are in the engine
 * list, its language is the page's. Downloading a language happens at
 * transcription time, never from here.
 */
export function OnDeviceSpeechDialog({ open, onOpenChange, status, language }: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  status: SpeechStatus;
  /** The page's language, as stored. */
  language: string | null;
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>On-device speech</DialogTitle>
          <DialogDescription>
            Apple's speech recogniser in macOS 26: free and offline, the audio never leaves this
            Mac, and an hour of it takes about a minute.
          </DialogDescription>
        </DialogHeader>
        <Body status={status} language={language} />
      </DialogContent>
    </Dialog>
  );
}

function Body({ status, language }: { status: SpeechStatus; language: string | null }) {
  if (status === undefined) return <p className="text-xs text-muted-foreground">Asking macOS…</p>;
  if (status === null || !status.available) {
    return (
      <p className="text-xs leading-relaxed text-muted-foreground">
        {status?.reason ?? "macOS did not say whether this Mac can transcribe on device."}
      </p>
    );
  }

  const locale = appleLocale(language, status.defaultLocale);
  const name = locale ? localeName(locale) : "English";
  const installed = status.installed.map(localeName).sort((a, b) => a.localeCompare(b));

  return (
    <div className="space-y-2 text-xs leading-relaxed text-muted-foreground">
      <p>
        Transcribes in <span className="text-foreground">{name}</span>
        {language === "auto"
          ? ": it has no auto-detect, so it keeps this Mac's English while the language is Auto-detect."
          : ", the language set on the Transcription page."}
        {locale && !status.installed.includes(locale)
          ? " Its model downloads once, the first time it is used."
          : ""}
      </p>
      <p>
        {installed.length
          ? `Downloaded on this Mac: ${installed.join(", ")}.`
          : "No language is downloaded yet; each downloads once, the first time it is used."}
      </p>
    </div>
  );
}
