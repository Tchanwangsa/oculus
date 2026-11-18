import { ArrowSquareOut } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { openLinkOut } from "@/lib/files/canvasLinks";

/** The strip over a challenge slide: Open in Ed, where the challenge is run
 *  and marked, its files here being a read-only copy of the student's
 *  workspace. An in-app tab; ⌘-click opens the system browser. */
export function ChallengeHeader({ edUrl }: { edUrl: string }) {
  return (
    <div className="flex h-10 shrink-0 items-center justify-end border-b border-border-subtle px-3">
      <Button size="xs" variant="secondary" onClick={(e) => void openLinkOut(edUrl, e.metaKey)}>
        <ArrowSquareOut />
        Open in Ed
      </Button>
    </div>
  );
}
