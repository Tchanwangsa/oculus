import { useEffect } from "react";
import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

/** The university profile as Rust reports it (`sources/institution`). */
export interface Institution {
  /** `https://host`, or null before the student enters an address. */
  canvas: string | null;
  host: string | null;
  /** Okta sign-in, so automatic sign-in can be offered. */
  okta: boolean;
  sso_host: string | null;
  /** A synced library pins the address until Start over. */
  locked: boolean;
}

interface InstitutionState {
  /** `undefined` until the first read. */
  institution: Institution | undefined;
  /** Why the last read failed; cleared by the next one that succeeds. */
  loadError: string | null;
  /** Reads the profile; on failure records `loadError` and rejects. */
  load: () => Promise<Institution>;
  /** Checks the address is Canvas, detects its sign-in and saves it; a
   *  changed address signs the app out. Rejects with Rust's reason. */
  configure: (address: string) => Promise<Institution>;
}

export const useInstitutionStore = create<InstitutionState>((set) => ({
  institution: undefined,
  loadError: null,
  load: async () => {
    try {
      const institution = await invoke<Institution>("institution_get");
      set({ institution, loadError: null });
      return institution;
    } catch (e) {
      set({ loadError: String(e) });
      throw e;
    }
  },
  configure: async (address) => {
    const institution = await invoke<Institution>("institution_configure", { address });
    set({ institution });
    return institution;
  },
}));

/** The profile, read once per app run by whichever component asks first. A
 *  failed read is not repeated on its own: `loadError` holds why, and a retry
 *  is a `load()` call. */
export function useInstitution(): Institution | undefined {
  const institution = useInstitutionStore((s) => s.institution);
  const loadError = useInstitutionStore((s) => s.loadError);
  const load = useInstitutionStore((s) => s.load);
  useEffect(() => {
    if (institution === undefined && loadError === null) load().catch(console.error);
  }, [institution, loadError, load]);
  return institution;
}

/** The current profile, reading it if no component has yet. */
export async function currentInstitution(): Promise<Institution> {
  return useInstitutionStore.getState().institution ?? useInstitutionStore.getState().load();
}
