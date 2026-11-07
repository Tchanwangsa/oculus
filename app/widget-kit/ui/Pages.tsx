import { Children, useState, type ReactNode } from "react";
import { Panels, clampIndex } from "./Panels";
import { Pill } from "./Pill";

/**
 * Content too tall for one frame, a page at a time: Back/Next pills and a
 * dot per page. Every page is laid out at once (`Panels`), so the frame is
 * the tallest page's height from the start and paging never resizes it.
 * Controlled with `page`/`onPageChange` (pair it with `useWidgetState` to
 * keep the page), or left to itself.
 */
export function Pages({
  children,
  page,
  onPageChange,
}: {
  children?: ReactNode;
  page?: number;
  onPageChange?: (page: number) => void;
}) {
  const count = Children.toArray(children).length;
  const [own, setOwn] = useState(0);
  const current = clampIndex(page ?? own, count);
  const go = (next: number) => {
    const to = clampIndex(next, count);
    if (page == null) setOwn(to);
    onPageChange?.(to);
  };
  return (
    <div className="o-pages">
      <Panels active={current}>{children}</Panels>
      {count > 1 && (
        <div className="o-pages-nav">
          <Pill tone="ghost" onClick={() => go(current - 1)} disabled={current === 0}>
            Back
          </Pill>
          <div className="o-dots" role="tablist" aria-label="Pages">
            {Array.from({ length: count }, (_, i) => (
              <button
                key={i}
                type="button"
                role="tab"
                aria-selected={i === current}
                aria-label={`Page ${i + 1} of ${count}`}
                className="o-dot"
                onClick={() => go(i)}
              />
            ))}
          </div>
          <Pill tone="ghost" onClick={() => go(current + 1)} disabled={current === count - 1}>
            Next
          </Pill>
        </div>
      )}
    </div>
  );
}
