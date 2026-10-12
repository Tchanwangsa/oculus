import type { ReactNode } from "react";
import { ArrowSquareOut } from "@phosphor-icons/react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ProviderMark } from "@/components/icons/ProviderMark";
import { Button } from "@/components/ui/button";
import { providerLabel, type Provider } from "@/lib/harness";
import { agentOption } from "@/lib/onboarding/agentOptions";
import { OptionCard } from "../OptionCard";

/** One agent on the Agent step: its logo and name, what it costs, the
 *  caller's status chip and actions, then a link to the vendor's plans.
 *  The link opens itself: `AppLayout`'s link handler isn't mounted during
 *  setup. */
export function AgentCard({
  provider,
  chip,
  actions,
  hint,
  ready,
}: {
  provider: Provider;
  chip?: ReactNode;
  actions?: ReactNode;
  /** A muted line under the plan note. */
  hint?: ReactNode;
  ready: boolean;
}) {
  const option = agentOption(provider);
  return (
    <OptionCard
      icon={<ProviderMark provider={provider} />}
      title={providerLabel(provider)}
      chip={chip}
      selected={ready}
      detail={
        <>
          <span className="block">{option.cost}</span>
          <span className="mt-0.5 block text-muted-foreground/80">{option.note}</span>
          {hint && <span className="mt-0.5 block text-muted-foreground/80">{hint}</span>}
        </>
      }
      actions={
        <>
          {actions}
          <Button
            variant="ghost"
            size="xs"
            className="text-muted-foreground"
            onClick={() => void openUrl(option.link.url)}
          >
            {option.link.label}
            <ArrowSquareOut />
          </Button>
        </>
      }
    />
  );
}
