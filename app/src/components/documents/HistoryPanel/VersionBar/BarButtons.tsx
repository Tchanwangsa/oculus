import { ArrowLeft, ListBullets } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

/** The bar's first control in every state: leaves History for the note. */
export function BackButton({ onClick }: { onClick: () => void }) {
  return (
    <Button
      variant="ghost"
      size="icon-xs"
      aria-label="Back to note"
      title="Back to note"
      className="-ml-1 shrink-0 text-muted-foreground"
      onClick={onClick}
    >
      <ArrowLeft size={14} />
    </Button>
  );
}

/** Shows or hides the versions sidebar; icon-only when the bar is short of room. */
export function VersionsToggle({ shown, onToggle }: { shown: boolean; onToggle: () => void }) {
  return (
    <Button
      variant="ghost"
      size="xs"
      aria-pressed={shown}
      aria-label="Versions"
      title={shown ? "Hide versions" : "Show versions"}
      onClick={onToggle}
      className={cn("shrink-0", shown ? "text-foreground" : "text-muted-foreground")}
    >
      <ListBullets />
      <span className="@max-[28rem]/bar:hidden">Versions</span>
    </Button>
  );
}
