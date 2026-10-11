import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

export function IconAction({
  label,
  onClick,
  disabled,
  size = "icon-xs",
  children,
}: {
  label: string;
  onClick: () => void;
  disabled?: boolean;
  size?: "icon-xs" | "icon-sm";
  children: React.ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="ghost"
          size={size}
          onClick={onClick}
          disabled={disabled}
          aria-label={label}
          className="shrink-0"
        >
          {children}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}
