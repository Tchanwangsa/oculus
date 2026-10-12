import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAuth, type AuthStatus } from "@/hooks/sync/useAuth";

export type CanvasState = AuthStatus | "checking";

/**
 * `useAuth`, with "checking" until its first answer, and its `error`. `useAuth` starts at
 * "disconnected" before `get_auth_status` replies, so a signed-in student
 * would see "Not connected" flash. Settled once that check says no, or once
 * `useAuth` leaves "disconnected" (a yes turns into "connected" there).
 */
export function useCanvasConnection() {
  const { status, error, connect } = useAuth();
  const [settled, setSettled] = useState(false);
  if (!settled && status !== "disconnected") setSettled(true);

  useEffect(() => {
    invoke<boolean>("get_auth_status")
      .then((ok) => {
        if (!ok) setSettled(true);
      })
      .catch(() => setSettled(true));
  }, []);

  const state: CanvasState = settled ? status : "checking";
  return { state, error, connect };
}
