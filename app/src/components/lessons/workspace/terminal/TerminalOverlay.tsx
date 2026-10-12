import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";

import type { useTerminalSession } from "./useTerminalSession";

/** What stands in for the terminal before it opens. */
export function TerminalOverlay({
  terminal,
  offline,
}: {
  terminal: ReturnType<typeof useTerminalSession>;
  offline: string | null;
}) {
  if (terminal.phase === "failed") {
    return (
      <div className="absolute inset-x-0 top-0 px-3 pt-2">
        <Alert variant="destructive">
          <AlertTitle className="text-xs">Couldn't open the terminal</AlertTitle>
          <AlertDescription className="text-xs">{terminal.error}</AlertDescription>
        </Alert>
      </div>
    );
  }
  if (terminal.phase !== "idle" && terminal.phase !== "opening") return null;
  return (
    <p className="absolute inset-x-0 top-0 px-3 pt-2 text-xs text-muted-foreground">
      {terminal.phase === "opening" ? "Starting the terminal…" : (offline ?? "Starting the terminal…")}
    </p>
  );
}
