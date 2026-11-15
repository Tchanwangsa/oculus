import { memo, useMemo } from "react";
import { parseErrorMeta, parseToolMeta, thoughtMs, type HarnessItem, type Provider } from "@/lib/harness";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { CLI_ICON, ErrorRow, RowShell, ThinkingRow, ToolRow, TOOL_ICON } from "./WorkRow";
import { Compacted, Continued, DeliveryNotice, FoldedReply, Reply, Stopped, Unprompted, User } from "./Messages";
import { PermissionCard } from "./PermissionCard";
import { RequestRow } from "./request/RequestRow";
import { bundleLabel, type ReplyRole } from "./rows";
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
  role,
  folded,
  stream,
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
  /** For a reply: how its run settled it (`buildRows`). */
  role?: ReplyRole;
  /** Inside a run's fold: a reply is work, drawn without its widget fences. */
  folded?: boolean;
  /** The reply still streaming, drawn as the row it will become (`Timeline`). */
  stream?: boolean;
}) {
  switch (item.kind) {
    case "user":
      return <User item={item} actions={actions} />;
    case "assistant":
      return folded ? <FoldedReply item={item} /> : <Reply item={item} asked={asked} actions={actions} role={role} stream={stream} />;
    case "thinking":
      return <ThinkingRow text={item.content ?? ""} ms={thoughtMs(item)} dim={dim} />;
    case "tool":
      return <Tool item={item} dim={dim} />;
    case "error":
      return (
        <ErrorRow text={item.content ?? ""} auth={parseErrorMeta(item).auth} onSignIn={onSignIn} />
      );
    case "interrupted":
      return <Stopped />;
    case "unprompted":
      return <Unprompted item={item} />;
    case "continuation":
      return <Continued item={item} />;
    case "delivery_notice":
      return <DeliveryNotice item={item} />;
    case "compaction":
      return <Compacted />;
    case "permission":
      return <PermissionCard item={item} actionable={!!latest} onFollowUp={actions?.followUp} />;
    case "request":
      return <RequestRow item={item} />;
  }
});

/** Two or more steps under one summary header; opened, the steps line up
 *  with the header rather than nesting under it. */
export const Bundle = memo(function Bundle({
  items,
  dim = true,
}: {
  items: HarnessItem[];
  /** Fade it until hovered; off inside a run's fold, whose rows read alike. */
  dim?: boolean;
}) {
  const { label, icon } = useMemo(() => bundleLabel(items), [items]);
  return (
    <RowShell
      icon={icon == null ? undefined : "cli" in icon ? CLI_ICON[icon.cli] : TOOL_ICON[icon.tool]}
      title={label}
      expandable
      dim={dim}
      flush
    >
      <div className="flex flex-col">
        {items.map((i) => (
          <Item key={i.id} item={i} dim={false} />
        ))}
      </div>
    </RowShell>
  );
}, (prev, next) => prev.dim === next.dim && prev.items.length === next.items.length &&
  prev.items.every((item, i) => item === next.items[i]));
