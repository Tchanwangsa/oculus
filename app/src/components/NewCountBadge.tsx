/**
 * Count pill for never-opened files (sidebar rows, subject tabs); nothing at
 * zero. A quiet brand chip, not a solid fill, so it doesn't drown the nav.
 */
export function NewCountBadge({ count }: { count: number }) {
  if (count <= 0) return null;
  return (
    <span className="shrink-0 flex h-[17px] min-w-[17px] items-center justify-center rounded-full border border-brand/20 bg-brand-muted px-1 text-[10px] font-medium leading-none tabular-nums text-brand">
      {count > 99 ? "99+" : count}
    </span>
  );
}
