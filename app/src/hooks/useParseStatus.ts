import { useEffect, useRef, useState } from "react";

export type ParseStatus = "idle" | "running" | "done" | "error" | "unavailable";

const SIDECAR = "http://127.0.0.1:9547";
const POLL_MS = 2000;

export function useParseStatus(pdfAbsPath: string | null): ParseStatus {
  const [status, setStatus] = useState<ParseStatus>("idle");
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (!pdfAbsPath) {
      setStatus("idle");
      return;
    }

    let cancelled = false;

    async function poll() {
      if (cancelled) return;
      try {
        const res = await fetch(
          `${SIDECAR}/parse-status?pdf_path=${encodeURIComponent(pdfAbsPath!)}`,
        );
        if (cancelled) return;
        if (!res.ok) {
          setStatus("unavailable");
          return;
        }
        const data = await res.json();
        const s: string = data.quality_status ?? "unknown";
        if (s === "done") {
          setStatus("done");
        } else if (s.startsWith("error")) {
          setStatus("error");
        } else if (s === "running") {
          setStatus("running");
          timerRef.current = setTimeout(poll, POLL_MS);
        } else {
          setStatus("unavailable");
        }
      } catch {
        if (!cancelled) setStatus("unavailable");
      }
    }

    setStatus("running");
    poll();

    return () => {
      cancelled = true;
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, [pdfAbsPath]);

  return status;
}
