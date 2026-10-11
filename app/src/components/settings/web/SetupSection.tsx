import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Section } from "@/components/settings/shared/section";
import { reopenOnboarding } from "@/stores/shell/onboardingStore";

/** Settings → Canvas: reopen first-run setup in place of the shell
 *  (docs/onboarding.md). */
export function SetupSection() {
  const [error, setError] = useState<string | null>(null);
  const reopen = () => {
    setError(null);
    reopenOnboarding().catch((e) => setError(`Couldn't open setup: ${String(e)}`));
  };

  return (
    <Section title="Setup">
      <div className="flex items-center justify-between gap-4 py-2">
        <div className="min-w-0">
          <div className="text-[13px] text-foreground">Run setup again</div>
          <div className="mt-0.5 text-xs text-muted-foreground">
            Walk through connecting Canvas, the library keys and an agent.
          </div>
        </div>
        <Button variant="outline" size="xs" className="shrink-0" onClick={reopen}>
          Open setup
        </Button>
      </div>
      {error && <p className="py-2 text-xs text-destructive">{error}</p>}
    </Section>
  );
}
