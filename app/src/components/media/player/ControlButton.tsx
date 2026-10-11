import { cn } from "@/lib/utils";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

/** A control-bar button. Not shadcn `Button`: its ghost hover (`accent`)
 *  vanishes on video. */
export function ControlButton({
  label,
  onClick,
  disabled,
  active,
  children,
}: {
  label: string;
  onClick: () => void;
  disabled?: boolean;
  active?: boolean;
  children: React.ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          onClick={onClick}
          disabled={disabled}
          aria-label={label}
          className={cn(
            "relative inline-flex size-8 shrink-0 items-center justify-center rounded-full",
            "transition-colors hover:bg-white/15 hover:text-white",
            "disabled:pointer-events-none disabled:opacity-40",
            active ? "text-white" : "text-white/85",
          )}
        >
          {children}
          {/* An "on" mark: on a frame a lit icon reads the same as an unlit one. */}
          {active && (
            <span className="absolute bottom-[3px] h-[2px] w-3.5 rounded-full bg-white" />
          )}
        </button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}
