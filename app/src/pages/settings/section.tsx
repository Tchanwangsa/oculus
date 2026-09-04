/** A titled settings group — header + description on top, rows beneath.
 *  Flat, no cards: the section header does the separating. */
export function Section({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: React.ReactNode;
}) {
  return (
    <section>
      <h2 className="text-[15px] font-semibold text-foreground">{title}</h2>
      {description && (
        <p className="text-xs text-muted-foreground mt-0.5">{description}</p>
      )}
      <div className="mt-3">{children}</div>
    </section>
  );
}

/** One label/value line, matching Linear's settings rows. */
export function StatRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between py-2">
      <span className="text-xs text-muted-foreground">{label}</span>
      <span className="text-xs text-foreground tabular-nums">{value}</span>
    </div>
  );
}
