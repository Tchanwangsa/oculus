import { useEffect, useState } from "react";

/** Restore once per mount; persist through the same state path as React.
 *  Readers own validation and defaults, keeping existing storage formats. */
export function useStoredState<T>(
  key: string,
  read: (raw: string | null) => T,
  write: (value: T) => string = String,
) {
  const [value, setValue] = useState(() => {
    let raw: string | null = null;
    try { raw = localStorage.getItem(key); } catch { /* Use the reader's default. */ }
    return read(raw);
  });
  useEffect(() => {
    try { localStorage.setItem(key, write(value)); } catch { /* State still works without storage. */ }
  }, [key, value, write]);
  return [value, setValue] as const;
}

/** Malformed JSON and non-string entries never hide a group. */
export function readStringSet(raw: string | null): Set<string> {
  try {
    const values: unknown = JSON.parse(raw ?? "[]");
    return new Set(Array.isArray(values) ? values.filter((v): v is string => typeof v === "string") : []);
  } catch {
    return new Set();
  }
}

const writeStringSet = (value: Set<string>) => JSON.stringify([...value]);

/** Store collapsed group keys, so newly discovered groups start expanded. */
export function useStoredSet(key: string) {
  // A blocked storage read is treated like an empty saved group selection.
  return useStoredState(key, readStringSet, writeStringSet);
}
