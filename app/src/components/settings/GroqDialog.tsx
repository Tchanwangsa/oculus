import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ArrowSquareOut } from "@phosphor-icons/react";
import { CredentialField } from "./CredentialField";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { deleteGroqKey, saveGroqKey } from "@/lib/transcribe";

/** Where a Groq key is created. Linked, not described. */
const GROQ_KEYS = "https://console.groq.com/keys";

/**
 * Groq's API key. Saving checks it against Groq's model list, which is free —
 * Settings never transcribes.
 */
export function GroqDialog({ open, onOpenChange, connected, onConnected }: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** null until the keychain has answered. */
  connected: boolean | null;
  onConnected: (connected: boolean) => void;
}) {
  const [key, setKey] = useState("");
  const [note, setNote] = useState<{ kind: "error" | "warn"; text: string } | null>(null);
  const [checking, setChecking] = useState(false);

  const save = async () => {
    if (!key.trim()) return;
    setChecking(true);
    setNote(null);
    try {
      const verdict = await saveGroqKey(key.trim());
      setKey("");
      onConnected(true);
      setNote(
        verdict === "unverified"
          ? { kind: "warn", text: "Saved, but Groq was unreachable — it has not been checked." }
          : null,
      );
    } catch (cause) {
      setNote({ kind: "error", text: String(cause) });
    } finally {
      setChecking(false);
    }
  };

  const remove = async () => {
    setNote(null);
    try {
      await deleteGroqKey();
      setKey("");
      onConnected(false);
    } catch (cause) {
      console.error("Groq key removal failed", cause);
      setNote({ kind: "error", text: String(cause) });
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Groq</DialogTitle>
          <DialogDescription>
            Whisper Large v3 Turbo on Groq's servers: the audio is uploaded, in parts when it is
            too large for one upload.
          </DialogDescription>
        </DialogHeader>
        <div className="min-w-0">
          <CredentialField
            label="API key"
            value={key}
            connected={connected ?? false}
            busy={checking}
            placeholder="Paste key"
            onChange={setKey}
            onSave={() => void save()}
            onRemove={() => void remove()}
            note={note}
          >
            {connected === false ? (
              <button
                type="button"
                className="mt-1 inline-flex items-center gap-1 text-[11px] text-brand hover:underline"
                onClick={() => void openUrl(GROQ_KEYS)}
              >
                Create a key
                <ArrowSquareOut size={11} weight="bold" />
              </button>
            ) : null}
          </CredentialField>
          <p className="mt-2 text-[11px] leading-relaxed text-muted-foreground">
            Groq's free tier caps the audio it takes an hour and a day; past that, the next engine
            in the list answers.
          </p>
        </div>
      </DialogContent>
    </Dialog>
  );
}
