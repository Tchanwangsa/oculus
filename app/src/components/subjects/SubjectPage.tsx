import type { ReactNode } from "react";
import { cn } from "@/lib/utils";
import { SkeletonRows } from "@/components/ui/PageParts";

/** Subject tabs share one gutter and width, including their loading view. */
export function SubjectPage({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <div className="page-scroll">
      <div className={cn("mx-auto max-w-5xl px-6 py-5", className)}>{children}</div>
    </div>
  );
}

export function SubjectLoading({ count, rowClassName, spacing }: {
  count: number;
  rowClassName?: string;
  spacing?: string;
}) {
  return (
    <SubjectPage className="py-6">
      <SkeletonRows count={count} rowClassName={rowClassName} className={spacing} />
    </SubjectPage>
  );
}

export function SubjectEmpty({ icon, title, children }: {
  icon: ReactNode;
  title: string;
  children?: ReactNode;
}) {
  return (
    <div className="h-full flex flex-col items-center justify-center gap-2">
      {icon}
      <p className="text-sm text-muted-foreground">{title}</p>
      {children}
    </div>
  );
}
