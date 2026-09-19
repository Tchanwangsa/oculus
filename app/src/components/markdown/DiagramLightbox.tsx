import { Lightbox, type LightboxSize } from "@/components/ui/Lightbox";

export type DiagramSize = LightboxSize;

/**
 * A diagram, opened out: the whole window, zoomable and pannable.
 *
 * The inline figure in a reply is a picture sized to the column it is in
 * (`Mermaid.tsx`), which is right for reading past and wrong for reading
 * *into* — a twenty-node flowchart in a 360px dock is a shape, not a diagram.
 * This is where it becomes one.
 *
 * The viewer itself is `components/ui/Lightbox.tsx`, shared with the pictures
 * a question attaches. What is left here is the two things that are true of a
 * diagram and of nothing else: the SVG goes in as markup, and its labels are
 * text the reader should be able to select through a pan.
 */
export function DiagramLightbox({
  svg,
  size,
  open,
  onOpenChange,
}: {
  /** The rendered SVG, already re-scoped by the caller so its ids cannot
   *  collide with the copy still on screen in the reply. */
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
      // The affordance has to match what the pointer will actually do: an
      // I-beam and a real selection over label text, the grab hand everywhere
      // else. SVG `<text>` is not selectable here by default at all — `body`
      // in `index.css` turns selection off app-wide — so saying so is what
      // makes "select when hovering over text" true.
      scrollerClassName="[&_svg_text]:cursor-text [&_svg_text]:select-text [&_svg_foreignObject]:cursor-text [&_svg_foreignObject]:select-text"
      // Mermaid draws labels as SVG `<text>`/`<tspan>` where it can
      // (`htmlLabels: false` in `Mermaid.tsx`) and as HTML inside a
      // `<foreignObject>` for the diagram types with no such switch, so both
      // shapes are named.
      selectableSelector="text, tspan, foreignObject"
    >
      {/* The `!` on `max-w-none` is not tidying: mermaid writes its natural
          width as an **inline** `max-width`, and an inline style outranks any
          class, so without the bang the picture stops growing at natural size
          however far it is zoomed. */}
      <div
        className="h-full w-full [&>svg]:h-full [&>svg]:w-full [&>svg]:max-w-none!"
        dangerouslySetInnerHTML={{ __html: svg }}
      />
    </Lightbox>
  );
}
