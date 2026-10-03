import { lazy, Suspense, useState } from "react";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { loadIconCatalogue } from "@/components/subjects/SubjectIcon";

// Through the glyphs' loader, so the first icon picked draws from the cache.
const PickerBody = lazy(() =>
  Promise.all([loadIconCatalogue(), import("@/components/subjects/SubjectIconPickerBody")])
    .then(([, body]) => body),
);

/** Subject identities keep the full icon catalogue off the default startup path. */
export function SubjectIconPicker({
  code,
  children,
}: {
  code: string;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(false);
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>{children}</PopoverTrigger>
      <PopoverContent align="start" sideOffset={8} className="w-[19rem] p-0">
        {open && (
          /* The body's height: 43px filter row, 38px swatch row, and the
             grid's `max-h-52` (208px), so the popover opens at its size. */
          <Suspense
            fallback={
              <p className="flex h-[289px] items-center justify-center text-[12px] text-muted-foreground">
                Loading icons…
              </p>
            }
          >
            <PickerBody code={code} />
          </Suspense>
        )}
      </PopoverContent>
    </Popover>
  );
}
