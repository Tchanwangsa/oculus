/**
 * The notification-count pill for new (never-opened) files, used on sidebar
 * subject rows and the subject tab strip. Renders nothing at zero.
 */
export function NewCountBadge({ count }: { count: number }) {
  if (count <= 0) return null;
  return (
    <span className="shrink-0 flex h-4 min-w-4 items-center justify-center rounded-full bg-primary px-1 text-[10px] font-medium leading-none tabular-nums text-primary-foreground">
      {count > 99 ? "99+" : count}
    </span>
  );
}
