import * as React from "react"
import { X } from "@phosphor-icons/react"
import { Dialog as DialogPrimitive } from "radix-ui"

import { cn } from "@/lib/utils"
import { Button } from "@/components/ui/button"

// Not stock shadcn any more. The generated primitive is drawn at shadcn's own
// scale — 18px title, 14px body, 24px padding, an 8px radius — which is a step
// up from everything around it in an app whose body text is 14px and whose
// popovers are `rounded-xl` on `border-border-subtle`. Every dialog in the app
// overrides only `max-w`, so the scale is fixed here once rather than at seven
// call sites.

function Dialog({
  ...props
}: React.ComponentProps<typeof DialogPrimitive.Root>) {
  return <DialogPrimitive.Root data-slot="dialog" {...props} />
}

function DialogTrigger({
  ...props
}: React.ComponentProps<typeof DialogPrimitive.Trigger>) {
  return <DialogPrimitive.Trigger data-slot="dialog-trigger" {...props} />
}

function DialogPortal({
  ...props
}: React.ComponentProps<typeof DialogPrimitive.Portal>) {
  return <DialogPrimitive.Portal data-slot="dialog-portal" {...props} />
}

function DialogClose({
  ...props
}: React.ComponentProps<typeof DialogPrimitive.Close>) {
  return <DialogPrimitive.Close data-slot="dialog-close" {...props} />
}

function DialogOverlay({
  className,
  ...props
}: React.ComponentProps<typeof DialogPrimitive.Overlay>) {
  return (
    <DialogPrimitive.Overlay
      data-slot="dialog-overlay"
      className={cn(
        "fixed inset-0 z-50 bg-black/50 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:animate-in data-[state=open]:fade-in-0",
        className
      )}
      {...props}
    />
  )
}

function DialogContent({
  className,
  children,
  showCloseButton = true,
  ...props
}: React.ComponentProps<typeof DialogPrimitive.Content> & {
  showCloseButton?: boolean
}) {
  return (
    <DialogPortal data-slot="dialog-portal">
      <DialogOverlay />
      <DialogPrimitive.Content
        data-slot="dialog-content"
        className={cn(
          // `[&>*]:min-w-0` is load-bearing, not tidying: this is a grid, and a
          // grid item's default `min-width: auto` refuses to shrink below its
          // min-content width. One unbreakable string in any child — an OAuth
          // URL, a long path — therefore sets the column's width, the card's
          // `max-w` is silently exceeded, and every *other* row re-wraps at
          // that new width while the footer slides off the right edge. A
          // `truncate` inside cannot save it; the ancestor has to be allowed
          // to shrink first.
          "fixed top-[50%] left-[50%] z-50 grid w-full max-w-[calc(100%-2rem)] translate-x-[-50%] translate-y-[-50%] gap-3.5 rounded-xl border border-border-subtle bg-popover p-5 shadow-lg duration-200 outline-none [&>*]:min-w-0 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95 data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95 sm:max-w-lg",
          className
        )}
        {...props}
      >
        {children}
        {showCloseButton && (
          <DialogPrimitive.Close
            data-slot="dialog-close"
            className="absolute top-4 right-4 rounded-xs opacity-70 ring-offset-background transition-opacity hover:opacity-100 focus:ring-2 focus:ring-ring focus:ring-offset-2 focus:outline-hidden disabled:pointer-events-none data-[state=open]:bg-accent data-[state=open]:text-muted-foreground [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4"
          >
            <X />
            <span className="sr-only">Close</span>
          </DialogPrimitive.Close>
        )}
      </DialogPrimitive.Content>
    </DialogPortal>
  )
}

// A dialog that *is* the window rather than a card floating in the middle of
// it — a lightbox. `DialogContent` above is a centred grid with a card's
// padding, radius, border and shadow, and overriding all of those at a call
// site to get a bare surface leaves more override than component; it also
// makes the note above ("every dialog overrides only `max-w`") stop being
// true. So the full-bleed case is its own export, composed from the same
// primitives and living in the same file, which is where this app keeps them.
//
// Deliberately without the close button: a canvas puts its own controls where
// the content is, not in a fixed corner. `DialogTitle` is still required by
// Radix for the announcement — give it `sr-only`.
function DialogCanvas({
  className,
  children,
  ...props
}: React.ComponentProps<typeof DialogPrimitive.Content>) {
  return (
    <DialogPortal data-slot="dialog-portal">
      {/* Not `bg-black/50`: a canvas dims the app rather than blacking it out,
          and the app's own ground is what it should dim towards in either
          theme. Nearly opaque, and blurred behind that — at 85% the text of
          the page underneath was still readable through the picture on top of
          it, which is the one thing a lightbox exists to stop. */}
      <DialogOverlay className="bg-background/95 backdrop-blur-sm" />
      <DialogPrimitive.Content
        data-slot="dialog-canvas"
        className={cn(
          "fixed inset-0 z-50 flex flex-col outline-none duration-200 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:animate-in data-[state=open]:fade-in-0",
          className,
        )}
        {...props}
      >
        {children}
      </DialogPrimitive.Content>
    </DialogPortal>
  )
}

function DialogHeader({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="dialog-header"
      className={cn("flex flex-col gap-1.5 text-center sm:text-left", className)}
      {...props}
    />
  )
}

function DialogFooter({
  className,
  showCloseButton = false,
  children,
  ...props
}: React.ComponentProps<"div"> & {
  showCloseButton?: boolean
}) {
  return (
    <div
      data-slot="dialog-footer"
      className={cn(
        "flex flex-col-reverse gap-2 sm:flex-row sm:justify-end",
        className
      )}
      {...props}
    >
      {children}
      {showCloseButton && (
        <DialogPrimitive.Close asChild>
          <Button variant="outline">Close</Button>
        </DialogPrimitive.Close>
      )}
    </div>
  )
}

function DialogTitle({
  className,
  ...props
}: React.ComponentProps<typeof DialogPrimitive.Title>) {
  return (
    <DialogPrimitive.Title
      data-slot="dialog-title"
      className={cn("text-base leading-snug font-semibold", className)}
      {...props}
    />
  )
}

function DialogDescription({
  className,
  ...props
}: React.ComponentProps<typeof DialogPrimitive.Description>) {
  return (
    <DialogPrimitive.Description
      data-slot="dialog-description"
      className={cn("text-[13px] text-muted-foreground", className)}
      {...props}
    />
  )
}

export {
  Dialog,
  DialogCanvas,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogOverlay,
  DialogPortal,
  DialogTitle,
  DialogTrigger,
}
