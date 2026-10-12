import { useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface KeyNote {
  kind: "error" | "warn";
  text: string;
}

export interface CloudKeyOptions {
  /** The service's name in the notes and logs: "MinerU", "Voyage". */
  service: string;
  /** Checks the key against the service, then stores it; answers `"ok"` or
   *  `"unverified"` (the service was unreachable and it was stored on trust). */
  setCommand: string;
  deleteCommand: string;
  /** After a key is stored and the field cleared, before the note is set. */
  onSaved: () => void;
  /** After the key is deleted and the field cleared. */
  onRemoved: () => void;
}

/**
 * One API key's paste field, shared by Settings and onboarding: the typed
 * text, the note under it, the busy flag while the service checks it, and
 * save/remove. Where "is a key saved" comes from, and what else a save
 * changes, belongs to the provider's wrapper (`useMineruKey`, `useVoyageKey`).
 */
export function useCloudKey({ service, setCommand, deleteCommand, onSaved, onRemoved }: CloudKeyOptions) {
  const [key, setKey] = useState("");
  const [note, setNote] = useState<KeyNote | null>(null);
  const [busy, setBusy] = useState(false);

  // Read at call time, so inline callbacks from the caller never go stale.
  const callbacks = useRef({ onSaved, onRemoved });
  callbacks.current = { onSaved, onRemoved };

  const save = async () => {
    if (!key.trim()) return;
    setBusy(true);
    setNote(null);
    try {
      const verdict = await invoke<string>(setCommand, { key: key.trim() });
      setKey("");
      callbacks.current.onSaved();
      setNote(
        verdict === "unverified"
          ? { kind: "warn", text: `Saved, but ${service} was unreachable — it has not been checked.` }
          : null,
      );
    } catch (cause) {
      setNote({ kind: "error", text: String(cause) });
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    setNote(null);
    try {
      await invoke(deleteCommand);
      setKey("");
      callbacks.current.onRemoved();
    } catch (cause) {
      console.error(`${service} key removal failed`, cause);
      setNote({ kind: "error", text: String(cause) });
    }
  };

  return { key, setKey, note, busy, save, remove };
}

export type CloudKey = ReturnType<typeof useCloudKey>;
