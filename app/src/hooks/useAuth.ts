import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTauriEvent } from "@/hooks/useEvents";

export type AuthStatus = "connected" | "disconnected" | "pending" | "expired";

const RECHECK_MS = 5 * 60_000;

export function useAuth() {
  const [status, setStatus] = useState<AuthStatus>("disconnected");

  useEffect(() => {
    invoke<boolean>("get_auth_status").then((ok) => {
      if (ok) setStatus("connected");
    });
  }, []);

  // The flag file only says a sign-in once happened, so ping Canvas to catch
  // expiry; "unreachable" (offline, 5xx) keeps the current state.
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

  useTauriEvent("canvas-auth-success", () => setStatus("connected"));
  useTauriEvent("canvas-auth-cancelled", () =>
    setStatus((p) => (p === "pending" ? "disconnected" : p)),
  );
  useTauriEvent("canvas-auth-expired", () => setStatus("expired"));

  /** Tries headless Okta sign-in with stored credentials first; anything it
   *  cannot answer (push, biometrics, no creds) falls through to the window. */
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
