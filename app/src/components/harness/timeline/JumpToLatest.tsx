import { ArrowDown } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";

export function JumpToLatest({ show, onClick }: { show: boolean; onClick: () => void }) {
  return (
    <div
      className={`pointer-events-none absolute inset-x-0 bottom-3 z-10 flex justify-center transition-opacity duration-150 ease-out motion-reduce:transition-none ${show ? "opacity-100" : "opacity-0"}`}
      aria-hidden={!show}
      inert={!show}
    >
      <Button
        type="button"
        variant="outline"
        size="icon"
        className={`bg-popover/95 text-foreground shadow-sm backdrop-blur-md hover:bg-popover dark:bg-popover/95 dark:hover:bg-popover ${show ? "pointer-events-auto" : "pointer-events-none"}`}
        tabIndex={show ? 0 : -1}
        aria-label="Jump to latest message"
        title="Jump to latest message"
        onClick={onClick}
      >
        <ArrowDown />
      </Button>
    </div>
  );
}
