import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type AuthStatus = "connected" | "disconnected" | "pending" | "expired";

const RECHECK_MS = 5 * 60_000;

export function useAuth() {
  const [status, setStatus] = useState<AuthStatus>("disconnected");

  useEffect(() => {
    invoke<boolean>("get_auth_status").then((ok) => {
      if (ok) setStatus("connected");
    });
  }, []);

  // The flag file only says a sign-in once happened. Ping Canvas now, on
  // window focus, and every few minutes so an expired session shows as
  // expired instead of a stale "Active". "unreachable" (network down, Canvas
  // 5xx) keeps the current state — no scaring people offline.
  useEffect(() => {
    let stopped = false;
    const check = async () => {
      try {
        const result = await invoke<string>("check_canvas_session");
        if (stopped || result === "unreachable") return;
        setStatus((p) => {
          if (p === "pending") return p;
          if (result === "valid") return "connected";
          return p === "disconnected" ? p : "expired";
        });
      } catch {
        /* command failed — keep current state */
      }
    };
    check();
    const timer = setInterval(check, RECHECK_MS);
    const onFocus = () => check();
    window.addEventListener("focus", onFocus);
    return () => {
      stopped = true;
      clearInterval(timer);
      window.removeEventListener("focus", onFocus);
    };
  }, []);

  useEffect(() => {
    const subs = [
      listen("canvas-auth-success", () => setStatus("connected")),
      listen("canvas-auth-cancelled", () =>
        setStatus((p) => (p === "pending" ? "disconnected" : p)),
      ),
      listen("canvas-auth-expired", () => setStatus("expired")),
    ];
    return () => {
      subs.forEach((p) => p.then((f) => f()));
    };
  }, []);

  /**
   * Sign in, preferring the stored-credential path: it answers the Okta
   * password and TOTP prompts in Rust, so nothing opens and nothing is typed.
   * Anything it cannot handle — no credentials saved, a factor we don't hold,
   * a changed password — falls through to the browser window, which is still
   * the only way to satisfy a push or biometric challenge.
   */
  const connect = async () => {
    setStatus("pending");
    try {
      await invoke<string>("okta_sign_in");
      setStatus("connected");
      return;
    } catch (err) {
      console.warn("[oculus] headless sign-in unavailable:", err);
    }
    try {
      await invoke("launch_canvas_auth");
    } catch {
      setStatus("disconnected");
    }
  };

  const disconnect = async () => {
    setStatus("disconnected");
    try {
      await invoke("disconnect_canvas");
    } catch {
      /* ignore */
    }
  };

  return { status, connect, disconnect };
}
