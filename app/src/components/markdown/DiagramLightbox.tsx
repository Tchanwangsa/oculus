import { Lightbox, type LightboxSize } from "@/components/ui/Lightbox";

export type DiagramSize = LightboxSize;

/**
 * A mermaid diagram full-window in `ui/Lightbox.tsx`: SVG as markup, with
 * selectable label text.
 */
export function DiagramLightbox({
  svg,
  size,
  open,
  onOpenChange,
}: {
  /** Ids already re-scoped so they don't collide with the inline copy. */
  svg: string;
  size: DiagramSize;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  return (
    <Lightbox
      size={size}
      open={open}
      onOpenChange={onOpenChange}
      title="Diagram"
      // `body` disables selection app-wide; labels opt back in.
      scrollerClassName="[&_svg_text]:cursor-text [&_svg_text]:select-text [&_svg_foreignObject]:cursor-text [&_svg_foreignObject]:select-text"
      // Labels are SVG text, or HTML in a `<foreignObject>` for diagram types
      // without `htmlLabels: false`.
      selectableSelector="text, tspan, foreignObject"
    >
      {/* `!` beats mermaid's inline `max-width`, which would cap the zoom. */}
      <div
        className="h-full w-full [&>svg]:h-full [&>svg]:w-full [&>svg]:max-w-none!"
        dangerouslySetInnerHTML={{ __html: svg }}
      />
    </Lightbox>
  );
}
