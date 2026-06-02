import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type AuthStatus = "connected" | "disconnected" | "pending";

export function useAuth() {
  const [status, setStatus] = useState<AuthStatus>("disconnected");

  useEffect(() => {
    invoke<boolean>("get_auth_status").then((ok) => {
      if (ok) setStatus("connected");
    });
  }, []);

  useEffect(() => {
    const subs = [
      listen("canvas-auth-success", () => setStatus("connected")),
      listen("canvas-auth-cancelled", () =>
        setStatus((p) => (p === "pending" ? "disconnected" : p)),
      ),
      listen("canvas-auth-expired", () => setStatus("disconnected")),
    ];
    return () => {
      subs.forEach((p) => p.then((f) => f()));
    };
  }, []);

  const connect = async () => {
    setStatus("pending");
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
