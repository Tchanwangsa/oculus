import { useRef, type ReactNode } from "react";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import { TaskSearch } from "./TaskSearch";

/** Search stays above the filter strip when this pane cannot fit both. */
export function TaskToolbar({ value, onChange, placeholder, children }: {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  children: ReactNode;
}) {
  const actions = useRef<HTMLDivElement>(null);
  useScrollFade(actions, "x");
  return (
    <div className="task-toolbar shrink-0 min-w-0">
      <div className="task-toolbar-layout">
        <div className="task-toolbar-search">
          <TaskSearch value={value} onChange={onChange} placeholder={placeholder} />
        </div>
        <div ref={actions} role="group" aria-label="List controls" className="task-toolbar-actions">
          <div className="task-toolbar-controls">{children}</div>
        </div>
      </div>
    </div>
  );
}
