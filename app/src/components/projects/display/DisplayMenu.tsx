import { useState } from "react";
import { CaretDown, SlidersHorizontal } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { DisplaySettings } from "./DisplaySettings";
import { PropertyPanel } from "./PropertyPanel";
import { SlidePanels } from "./SlidePanels";
import { DEFAULT_DISPLAY, type TasksDisplay } from "./settings";

/**
 * The Display pill and its popover: layout, grouping, sort, completed, and a
 * second panel for property visibility that slides in over the first. Closing
 * resets to the first panel with an empty search.
 */
export function DisplayMenu({
  display,
  onChange,
  defaults = DEFAULT_DISPLAY,
  showLayout = true,
}: {
  display: TasksDisplay;
  defaults?: TasksDisplay;
  showLayout?: boolean;
  onChange: (next: TasksDisplay) => void;
}) {
  const [open, setOpen] = useState(false);
  const [panel, setPanel] = useState<"first" | "second">("first");
  const [query, setQuery] = useState("");

  const onOpenChange = (next: boolean) => {
    setOpen(next);
    if (!next) {
      setPanel("first");
      setQuery("");
    }
  };

  return (
    <Popover open={open} onOpenChange={onOpenChange}>
      <PopoverTrigger asChild>
        <Button variant="outline" size="sm" className="shrink-0 gap-1.5 font-normal">
          <SlidersHorizontal size={13} className="text-muted-foreground" />
          Display
          <CaretDown size={10} weight="bold" className="text-muted-foreground" />
        </Button>
      </PopoverTrigger>
      <PopoverContent
        align="end"
        className="w-80 p-0"
        // A typed property search clears on Escape before the menu closes.
        onEscapeKeyDown={(e) => {
          if (panel === "second" && query) e.preventDefault();
        }}
      >
        <SlidePanels
          showing={panel}
          first={
            <DisplaySettings
              display={display}
              onChange={onChange}
              defaults={defaults}
              showLayout={showLayout}
              active={panel === "first"}
              onOpenProperties={() => setPanel("second")}
            />
          }
          second={
            <PropertyPanel
              display={display}
              onChange={onChange}
              query={query}
              onQuery={setQuery}
              active={panel === "second"}
              onBack={() => setPanel("first")}
              onClose={() => onOpenChange(false)}
            />
          }
        />
      </PopoverContent>
    </Popover>
  );
}
