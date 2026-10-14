import { lazy, Suspense, useState, type ReactNode } from "react";
import { Popover, PopoverContent, PopoverTrigger } from "../popover";
import { loadIconCatalogue } from "./CatalogueIcon";
import type { IconPreference } from "@/lib/ui/iconPreferences";

const PickerBody = lazy(() => Promise.all([loadIconCatalogue(), import("./IconPickerBody")]).then(([, body]) => body));

export interface IconPickerProps {
  children: ReactNode;
  value?: IconPreference;
  defaultName: string;
  defaultColor: string;
  weight?: "fill" | "regular";
  onChange: (patch: Partial<IconPreference>) => void;
  onReset: () => void;
}

/** The shared identity picker; callers own storage and automatic defaults. */
export function IconPicker({ children, ...props }: IconPickerProps) {
  const [open, setOpen] = useState(false);
  return <Popover open={open} onOpenChange={setOpen}>
    <PopoverTrigger asChild>{children}</PopoverTrigger>
    <PopoverContent align="start" sideOffset={8} className="w-[19rem] p-0">
      {open && <Suspense fallback={<p className="flex h-[289px] items-center justify-center text-[12px] text-muted-foreground">Loading icons…</p>}>
        <PickerBody {...props} />
      </Suspense>}
    </PopoverContent>
  </Popover>;
}
