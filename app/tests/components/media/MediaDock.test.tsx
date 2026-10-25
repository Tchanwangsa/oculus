import { expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { MediaDock } from "@/components/media/MediaDock";
import { TooltipProvider } from "@/components/ui/tooltip";

function render(draggable: boolean) {
  return renderToStaticMarkup(
    <TooltipProvider>
      <MediaDock
        tabs={[{ value: "chat", label: "Chat" }]}
        value="chat"
        onChange={() => {}}
        dock="bottom"
        size={260}
        open
        resizing={false}
        onClose={() => {}}
        onHeaderPointerDown={draggable ? () => {} : undefined}
      >
        Chat content
      </MediaDock>
    </TooltipProvider>,
  );
}

test("a fixed dock keeps its tabs and close control without advertising a drag", () => {
  const html = render(false);
  expect(html).toContain("Chat content");
  expect(html).toContain('aria-label="Hide panel"');
  expect(html).not.toContain("cursor-grab");
});

test("a movable dock advertises dragging", () => {
  const html = render(true);
  expect(html).toContain("cursor-grab");
});
