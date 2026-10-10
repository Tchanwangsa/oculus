import { Link } from "react-router-dom";
import { ArrowRight } from "@phosphor-icons/react";
import { filePageHref, openFileSmart } from "@/lib/files/openFile";
import { FileRecency } from "@/components/files/FileRecency";
import type { DbFile } from "@/lib/db";

export function Section({
  title, children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <section>
      <h2 className="mb-2.5 text-[13px] font-semibold text-foreground">{title}</h2>
      {children}
    </section>
  );
}

/** Opens the file in the side panel. */
export function FileLink({
  file, label, meta,
}: {
  file: DbFile;
  label: string;
  meta?: string;
}) {
  return (
    <button
      data-tab-href={filePageHref(file) ?? undefined}
      onClick={() => openFileSmart(file)}
      className="w-full flex items-center gap-3 rounded-md px-2 py-1.5 -mx-2 text-left text-muted-foreground hover:bg-surface hover:text-foreground transition-colors"
    >
      <span className="text-[12px] truncate flex-1">{label}</span>
      {meta && <span className="text-[11px] opacity-60 shrink-0">{meta}</span>}
      <FileRecency file={file} />
    </button>
  );
}

export function MoreLink({ to, label }: { to: string; label: string }) {
  return (
    <Link
      to={to}
      className="mt-2 inline-flex items-center gap-1 text-[11px] text-brand hover:underline"
    >
      {label}
      <ArrowRight size={10} />
    </Link>
  );
}
