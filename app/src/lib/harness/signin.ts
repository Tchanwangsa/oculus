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
  /** Rust opens it too; shown with Copy in case that `open` failed. */
  url: string | null;
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
