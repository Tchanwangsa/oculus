import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTauriEvent } from "@/hooks/backend/useEvents";
import { navigateActive } from "@/lib/shell/tabRouters";
import { currentInstitution } from "@/stores/sync/institutionStore";

export type AuthStatus = "connected" | "disconnected" | "pending" | "expired";

const RECHECK_MS = 5 * 60_000;

export function useAuth() {
  const [status, setStatus] = useState<AuthStatus>("disconnected");
  // Why the last connect could not open the sign-in window, shown in place.
  const [error, setError] = useState<string | null>(null);

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

  useTauriEvent("canvas-auth-success", () => {
    setStatus("connected");
    setError(null);
  });
  useTauriEvent("canvas-auth-cancelled", () =>
    setStatus((p) => (p === "pending" ? "disconnected" : p)),
  );
  useTauriEvent("canvas-auth-expired", () => setStatus("expired"));
  useTauriEvent("canvas-signed-out", () => setStatus("disconnected"));

  /** With no Canvas address yet, opens Settings → Canvas to enter one. At an
   *  Okta university it tries headless sign-in with stored credentials first;
   *  anything that cannot answer (push, biometrics, no creds, another IdP)
   *  falls through to the window. A window that cannot open (no keyd that
   *  knows the university, a university change in flight) sets `error`. */
  const connect = async () => {
    setError(null);
    const institution = await currentInstitution().catch(() => null);
    if (!institution?.canvas) {
      navigateActive("/settings/canvas");
      return;
    }
    setStatus("pending");
    if (institution.okta) {
      try {
        await invoke<string>("okta_sign_in");
        setStatus("connected");
        return;
      } catch (err) {
        console.warn("[oculus] headless sign-in unavailable:", err);
      }
    }
    try {
      await invoke("launch_canvas_auth");
    } catch (err) {
      setStatus("disconnected");
      setError(err instanceof Error ? err.message : String(err));
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

  return { status, error, connect, disconnect };
}
