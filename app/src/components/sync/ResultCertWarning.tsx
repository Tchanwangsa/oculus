import { useCallback, useEffect, useState } from "react";
import { Warning } from "@phosphor-icons/react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useTauriEvent } from "@/hooks/useEvents";
import { expiryDate, resultCertState, type ResultCertState } from "@/lib/resultCert";

/**
 * A warning icon while parse results download through MinerU's expired CDN
 * certificate. Re-read on every parse event: each download's handshake updates
 * the state, so the icon goes once MinerU renews.
 */
export function ResultCertWarning() {
  const [state, setState] = useState<ResultCertState | null>(null);

  const refresh = useCallback(() => {
    resultCertState()
      .then(setState)
      .catch((cause) => console.error("result certificate state failed", cause));
  }, []);

  useEffect(refresh, [refresh]);
  useTauriEvent("parse-status", refresh);

  if (!state?.bypassing) return null;
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span className="flex items-center gap-1 text-[11px] text-warning">
          <Warning size={13} weight="fill" />
          Expired certificate
        </span>
      </TooltipTrigger>
      <TooltipContent>
        MinerU's download server certificate expired {expiryDate(state)}. Oculus is accepting it
        anyway for parse results; everything else about it is still checked. Turn this off in
        Settings → Parsing.
      </TooltipContent>
    </Tooltip>
  );
}
