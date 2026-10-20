import { useEffect, useState } from "react";
import { errorMessage } from "@/lib/canvas/authority";
import { fileTitle } from "@/lib/files/openFile";
import { tauriSharingApi } from "@/lib/sharing/api";
import { sharedFile, stopAllShares, stopShare } from "@/lib/sharing/registry";
import { useSharing } from "@/lib/sharing/store";
import type { ShareEndpoint } from "@/lib/sharing/types";
import { SharingSettingsView } from "./SharingSettingsView";

/** Settings → Device sharing: what sharing is, and every live share with Stop.
 * Addresses come from the event (bare pages, no key); before the first event
 * they are read once from `companion_endpoints`. */
export function DeviceSharingSection() {
  const { state } = useSharing();
  const [endpoints, setEndpoints] = useState<ShareEndpoint[]>([]);
  const [stopping, setStopping] = useState<ReadonlySet<number>>(new Set());
  const [stoppingAll, setStoppingAll] = useState(false);
  const [confirmingAll, setConfirmingAll] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const hasState = state != null;

  useEffect(() => {
    if (hasState) return;
    let live = true;
    tauriSharingApi.endpoints().then(
      (read) => { if (live) setEndpoints(read.endpoints); },
      (failure: unknown) => { if (live) setError(`Could not read this Mac’s addresses: ${errorMessage(failure)}`); },
    );
    return () => { live = false; };
  }, [hasState]);

  const shares = (state?.shares ?? []).filter((s) => s.phase !== "ended").map((share) => {
    const file = sharedFile(share.fileId);
    return { share, title: file ? fileTitle(file) : `Canvas ${share.fileId}` };
  });

  const stop = async (fileId: number) => {
    setError(null);
    setStopping((s) => new Set(s).add(fileId));
    const failure = await stopShare(fileId);
    setStopping((s) => { const next = new Set(s); next.delete(fileId); return next; });
    if (failure) setError(failure.message);
  };
  const stopAll = async () => {
    setError(null);
    setConfirmingAll(false);
    setStoppingAll(true);
    const failure = await stopAllShares();
    setStoppingAll(false);
    if (failure) setError(failure.message);
  };

  return (
    <SharingSettingsView
      shares={shares} endpoints={state?.endpoints ?? endpoints} stopping={stopping} stoppingAll={stoppingAll}
      confirmingAll={confirmingAll} error={error}
      onStop={(fileId) => void stop(fileId)} onStopAll={() => void stopAll()} onConfirmAll={setConfirmingAll}
    />
  );
}
