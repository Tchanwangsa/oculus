import * as React from "react"

import { cn } from "@/lib/utils"

// `text-base md:text-sm` is stock shadcn's iOS fix — mobile Safari zooms the
// page when a focused field is under 16px — and it is a trap in a desktop app.
// The viewport is always past the `md` breakpoint, so the field is always 14px,
// and because Tailwind emits variant utilities *after* plain ones, `md:text-sm`
// outranks any `text-[13px]` a call site passes: the class sits in the DOM and
// does nothing. One unconditional size instead, so call sites can override it.

function Input({ className, type, ...props }: React.ComponentProps<"input">) {
  return (
    <input
      type={type}
      data-slot="input"
      className={cn(
        "h-8 w-full min-w-0 rounded-lg border border-input bg-transparent px-3.5 py-1 text-[13px] shadow-xs transition-[color,box-shadow] outline-none file:inline-flex file:h-7 file:border-0 file:bg-transparent file:text-sm file:font-medium file:text-foreground placeholder:text-muted-foreground disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 dark:bg-input/30",
        "focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/25",
        "aria-invalid:border-destructive aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40",
        className
      )}
      {...props}
    />
  )
}

export { Input }
