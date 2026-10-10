import { memo, useMemo } from "react";
import { parseErrorMeta, parseToolMeta, type HarnessItem, type Provider } from "@/lib/harness";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { ErrorRow, RowShell, ThinkingRow, ToolRow, TOOL_ICON } from "./WorkRow";
import { Reply, Stopped, User } from "./Messages";
import { PermissionCard } from "./PermissionCard";
import { bundleLabel } from "./rows";
import type { QuestionActions } from "./types";

/** A running tool's output streams in; it subscribes itself so only this row
 *  re-renders. */
function Tool({ item, dim }: { item: HarnessItem; dim: boolean }) {
  const done = parseToolMeta(item).ok != null;
  const ref = item.ref_id;
  const liveOutput = useHarnessStore((s) =>
    done || !ref ? undefined : s.live[item.thread_id]?.toolOutput[ref],
  );
  return <ToolRow item={item} dim={dim} liveOutput={liveOutput} />;
}

export const Item = memo(function Item({
  item,
  dim,
  actions,
  asked,
  onSignIn,
  latest,
}: {
  item: HarnessItem;
  dim: boolean;
  /** Stable for the life of the page, so memoising these rows still works. */
  actions?: QuestionActions;
  /** For an answer: the question above it (identity-stable with `items`). */
  asked?: HarnessItem;
  /** Opens the sign-in dialog; a `setState`, so stable. */
  onSignIn?: (provider: Provider) => void;
  /** For a `permission` row: the latest refusal carries the allow button. */
  latest?: boolean;
}) {
  switch (item.kind) {
    case "user":
      return <User item={item} actions={actions} />;
    case "assistant":
      return <Reply item={item} asked={asked} actions={actions} />;
    case "thinking":
      return <ThinkingRow text={item.content ?? ""} dim={dim} />;
    case "tool":
      return <Tool item={item} dim={dim} />;
    case "error":
      return (
        <ErrorRow text={item.content ?? ""} auth={parseErrorMeta(item).auth} onSignIn={onSignIn} />
      );
    case "interrupted":
      return <Stopped />;
    case "permission":
      return <PermissionCard item={item} actionable={!!latest} onFollowUp={actions?.followUp} />;
  }
});

export const Bundle = memo(function Bundle({ items }: { items: HarnessItem[] }) {
  const { label, icon } = useMemo(() => bundleLabel(items), [items]);
  return (
    <RowShell icon={TOOL_ICON[icon]} title={label} expandable dim>
      <div className="relative my-0.5 pl-3 before:absolute before:bottom-1 before:left-1.5 before:top-0 before:w-px before:bg-border before:content-['']">
        <div className="flex flex-col">
          {items.map((i) => (
            <Item key={i.id} item={i} dim={false} />
          ))}
        </div>
      </div>
    </RowShell>
  );
}, (prev, next) => prev.items.length === next.items.length &&
  prev.items.every((item, i) => item === next.items[i]));
