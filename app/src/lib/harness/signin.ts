import { invoke } from "@tauri-apps/api/core";
import type { Provider } from "./providers";

/** Read through `useSignInStatus` (Rust caches nothing). `signedIn: null` is
 *  "not answerable here" (opencode); `error` is the check itself failing. */
export interface SignInStatus {
  provider: Provider;
  signedIn: boolean | null;
  /** An email, else the route ("Claude subscription", "ChatGPT"). */
  account: string | null;
  error: string | null;
}

export const SIGNIN_EVENT = "harness-signin";

export interface SignInLine {
  provider: Provider;
  line: string | null;
  /** Opened in the browser already (by Rust, or by `agy` itself); shown
   *  with Copy and Open in case that failed. Antigravity sends a new one
   *  each time it goes round again after a refused code. */
  url: string | null;
  /** A problem the student must see while the run carries on: the
   *  sentence Antigravity refused a pasted code with. */
  notice: string | null;
  done: boolean;
  ok: boolean | null;
  status: string | null;
}

export function harnessSignInStatus(provider: Provider): Promise<SignInStatus> {
  return invoke<SignInStatus>("harness_sign_in_status", { provider });
}

/** Output streams on `SIGNIN_EVENT` until `done`. Rejects for opencode. */
export function harnessSignInStart(provider: Provider): Promise<void> {
  return invoke("harness_sign_in_start", { provider });
}

export function harnessSignInCode(provider: Provider, code: string): Promise<void> {
  return invoke("harness_sign_in_code", { provider, code });
}

export function harnessSignInCancel(provider: Provider): Promise<void> {
  return invoke("harness_sign_in_cancel", { provider });
}
