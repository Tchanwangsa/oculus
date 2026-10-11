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
  load: () => Promise<Institution>;
  /** Checks the address is Canvas, detects its sign-in and saves it; a
   *  changed address signs the app out. Rejects with Rust's reason. */
  configure: (address: string) => Promise<Institution>;
}

export const useInstitutionStore = create<InstitutionState>((set) => ({
  institution: undefined,
  load: async () => {
    const institution = await invoke<Institution>("institution_get");
    set({ institution });
    return institution;
  },
  configure: async (address) => {
    const institution = await invoke<Institution>("institution_configure", { address });
    set({ institution });
    return institution;
  },
}));

/** The profile, read once per app run by whichever component asks first. */
export function useInstitution(): Institution | undefined {
  const institution = useInstitutionStore((s) => s.institution);
  const load = useInstitutionStore((s) => s.load);
  useEffect(() => {
    if (institution === undefined) load().catch(console.error);
  }, [institution, load]);
  return institution;
}

/** The current profile, reading it if no component has yet. */
export async function currentInstitution(): Promise<Institution> {
  return useInstitutionStore.getState().institution ?? useInstitutionStore.getState().load();
}
