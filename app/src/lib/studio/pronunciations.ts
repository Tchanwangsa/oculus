import { invoke } from "@tauri-apps/api/core";

// The pronunciation review (`app/src-tauri/src/studio/commands/pronunciations.rs`):
// readings the Studio agent suggested for words Kokoro guessed, each kept
// until the student decides. Deciding writes the word into the subject's (or
// the global library's) lexicon.json; it applies at the video's next narrate.
// The samples are synthesised by Kokoro on this Mac, never a billed call.

export type PronunciationStatus = "pending" | "wrong";

/** Mirrors `Item`: a `Record` with its id. */
export interface Pronunciation {
  /** `<subject or _library>/<key>`. */
  id: string;
  /** The word as the script spells it. */
  word: string;
  /** The suggested lexicon value: a respelling or `/phonemes/`. */
  reading: string;
  /** The phonemes Kokoro read the word with when it was suggested. */
  original: string;
  /** Whether the guesser read it. */
  guessed: boolean;
  subject: string | null;
  /** The video's slug. */
  video: string;
  /** The spoken sentence the word is in: what the samples say. */
  sentence: string;
  /** The scene's id. */
  scene: string;
  /** The script.md line, 1-based. */
  line: number;
  status: PronunciationStatus;
  /** The student's note on a wrong suggestion. */
  note: string | null;
  /** `YYYY-MM-DDTHH:MM:SS`, UTC. */
  created: string;
}

/** `correct` writes the suggestion; `fine` pins the original guess. */
export type PronunciationDecision = "correct" | "fine";

/** Mirrors `Decided`. */
export interface PronunciationDecided {
  /** The lexicon.json written. */
  lexicon: string;
  word: string;
  /** The value written: the suggestion, or the original as `/phonemes/`. */
  reading: string;
}

/** Every suggestion, newest first. */
export function pronunciations(): Promise<Pronunciation[]> {
  return invoke<Pronunciation[]>("studio_pronunciations_list");
}

/** Writes the word into its lexicon and deletes the item; rejects with a
 *  sentence fit to show (a reading that does not check, an unreadable
 *  lexicon.json, the G2P pack missing). */
export function decidePronunciation(
  id: string,
  decision: PronunciationDecision,
): Promise<PronunciationDecided> {
  return invoke<PronunciationDecided>("studio_pronunciation_decide", { id, decision });
}

/** Marks the suggestion wrong; a blank note is stored as none. */
export function markPronunciationWrong(id: string, note: string | null): Promise<Pronunciation> {
  return invoke<Pronunciation>("studio_pronunciation_wrong", { id, note });
}

/** The item's sentence with the word read as its original guess or as the
 *  suggestion, in Settings -> Voice's Kokoro voice and speed, as WAV bytes;
 *  rejects with a sentence fit to show (no voice chosen, a pack missing). */
export function pronunciationSample(
  id: string,
  which: "original" | "suggestion",
): Promise<ArrayBuffer> {
  return invoke<ArrayBuffer>("studio_pronunciation_sample", { id, which });
}
