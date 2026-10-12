import { X } from "@phosphor-icons/react";
import { RowAction } from "@/components/ui/RowAction";

/**
 * "Remove from queue" inside an Up next row: shown on the row's hover (or
 * while focused), beside "Add to queue". `onRemove` sends the command; a
 * refusal shows as the player's error, like any command.
 */
export function RemoveAction({
  onRemove,
  disabled = false,
}: {
  onRemove: () => void;
  disabled?: boolean;
}) {
  return (
    <RowAction
      label="Remove from queue"
      onTrigger={() => {
        if (!disabled) onRemove();
      }}
      className="invisible flex size-6 shrink-0 items-center justify-center hover:text-foreground group-hover/row:visible group-focus-within/row:visible focus-visible:visible"
    >
      <X size={14} />
    </RowAction>
  );
}
