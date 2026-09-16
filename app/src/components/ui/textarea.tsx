import * as React from "react"

import { cn } from "@/lib/utils"

// `text-base md:text-sm` is stock shadcn's iOS fix — mobile Safari zooms the
// page when a focused field is under 16px — and it is a trap in a desktop app.
// The viewport is always past the `md` breakpoint, so the field is always 14px,
// and because Tailwind emits variant utilities *after* plain ones, `md:text-sm`
// outranks any `text-[13px]` a call site passes: the class sits in the DOM and
// does nothing. One unconditional size instead, so call sites can override it.

function Textarea({ className, ...props }: React.ComponentProps<"textarea">) {
  return (
    <textarea
      data-slot="textarea"
      className={cn(
        "flex field-sizing-content min-h-16 w-full rounded-lg border border-input bg-transparent px-3.5 py-2.5 text-[13px] shadow-xs transition-[color,box-shadow] outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/25 disabled:cursor-not-allowed disabled:opacity-50 aria-invalid:border-destructive aria-invalid:ring-destructive/20 dark:bg-input/30 dark:aria-invalid:ring-destructive/40",
        className
      )}
      {...props}
    />
  )
}

export { Textarea }
