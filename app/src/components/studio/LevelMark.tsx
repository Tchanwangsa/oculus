/**
 * Where a component or theme comes from: a subject's own version of a
 * global or built-in one (a brand pill), the subject's alone, the global
 * library's or built in. Lists in the global library pass `inSubject`
 * false and show nothing; pickers always show it.
 */
export function LevelMark({
  level,
  shadows,
  inSubject,
}: {
  level: "bundled" | "global" | "subject";
  /** The subject's file replaces a global (or bundled) one. */
  shadows: boolean;
  inSubject: boolean;
}) {
  if (!inSubject) return null;
  if (level === "subject" && shadows) {
    return (
      <span className="shrink-0 rounded-full bg-brand-muted px-1.5 py-px text-[10px] font-medium text-brand">
        Own version
      </span>
    );
  }
  return (
    <span className="shrink-0 text-[10.5px] text-muted-foreground">
      {level === "subject" ? "Subject" : level === "global" ? "Global" : "Built in"}
    </span>
  );
}

/** Marks a component of the starter set studio-core ships, wherever it lives
 *  now and whether or not it was edited. */
export function StarterMark() {
  return (
    <span className="shrink-0 rounded-full bg-secondary px-1.5 py-px text-[10px] font-medium text-muted-foreground">
      Starter
    </span>
  );
}
