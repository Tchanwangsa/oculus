import type { LocalProbe } from "./types";

/**
 * Whether the local server is answering, in one line. The failure sentence is
 * Rust's `detail`, which distinguishes cases the state alone cannot (e.g. a
 * server still loading models reads as `unreachable`).
 */
export function ProbeLine({
  probe,
  probing,
  probeError,
}: {
  probe: LocalProbe | null;
  probing: boolean;
  probeError: string | null;
}) {
  if (probing) {
    return <p className="text-[11px] text-muted-foreground">Checking…</p>;
  }
  if (probeError) {
    return (
      <p className="text-[11px] leading-relaxed text-destructive">
        Could not check the server: {probeError}
      </p>
    );
  }
  if (!probe) {
    return <p className="text-[11px] text-muted-foreground">Not checked yet.</p>;
  }
  if (probe.state === "reachable") {
    return (
      <p className="text-[11px] leading-relaxed text-success">
        Answering at {probe.base_url}.
      </p>
    );
  }
  return (
    <p className="text-[11px] leading-relaxed text-warning">
      {probe.detail ?? `Nothing answered at ${probe.base_url}.`}
    </p>
  );
}
