import { useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { cn } from "@/lib/utils";

/**
 * Two panels in one clipped box: the second slides in over the first and the
 * box eases to the height of whichever is showing. Both stay mounted, so state
 * (a search query) survives going back; the hidden one is `inert`. Heights are
 * `offsetHeight`, layout pixels, so page zoom does not skew them.
 */
export function SlidePanels({
  showing,
  first,
  second,
}: {
  showing: "first" | "second";
  first: ReactNode;
  second: ReactNode;
}) {
  const firstRef = useRef<HTMLDivElement>(null);
  const secondRef = useRef<HTMLDivElement>(null);
  const [heights, setHeights] = useState({ first: 0, second: 0 });

  useLayoutEffect(() => {
    const measure = () =>
      setHeights({
        first: firstRef.current?.offsetHeight ?? 0,
        second: secondRef.current?.offsetHeight ?? 0,
      });
    measure();
    const observer = new ResizeObserver(measure);
    if (firstRef.current) observer.observe(firstRef.current);
    if (secondRef.current) observer.observe(secondRef.current);
    return () => observer.disconnect();
  }, []);

  const panel = "absolute inset-x-0 top-0 transition-transform duration-200 ease-out will-change-transform";
  return (
    <div
      className="relative overflow-hidden rounded-xl transition-[height] duration-200 ease-out"
      style={{ height: heights[showing] || undefined }}
    >
      <div
        ref={firstRef}
        inert={showing !== "first"}
        className={cn(panel, showing !== "first" && "-translate-x-full")}
      >
        {first}
      </div>
      <div
        ref={secondRef}
        inert={showing !== "second"}
        className={cn(panel, showing !== "second" && "translate-x-full")}
      >
        {second}
      </div>
    </div>
  );
}
