import { useState } from "react";
import { useInstitutionStore, type Institution } from "@/stores/sync/institutionStore";

/** How the saved address signs in, in a few words. */
export function signInSummary(i: Institution): string {
  if (i.okta && i.sso_host) return `Signs in through Okta at ${i.sso_host}.`;
  if (i.sso_host) return `Signs in through ${i.sso_host}.`;
  return "Signs in on Canvas's own login page.";
}

/**
 * The Canvas address field's state, shared by Settings → Canvas and
 * onboarding's Canvas step. `save` hands the address to Rust, which checks it
 * is Canvas and detects its sign-in; a refusal lands in `error`.
 */
export function useCanvasAddressForm(onSaved?: () => void) {
  const configure = useInstitutionStore((s) => s.configure);
  const [address, setAddress] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const save = async () => {
    if (!address.trim() || busy) return;
    setBusy(true);
    setError(null);
    try {
      await configure(address);
      setAddress("");
      onSaved?.();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return { address, setAddress, busy, error, setError, save };
}
