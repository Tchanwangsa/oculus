import { CaretLeft, CaretRight, Check, PencilSimple, Trash, Warning, X } from "@phosphor-icons/react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { fmtClock, sqliteUtcToMs } from "@/lib/format/format";
import { type Row } from "./model";
import { displayLocale } from "@/lib/format/locale";

export function Pager({ page, count, onChange }: { page: number; count: number; onChange: (page: number) => void }) {
  if (count <= 5) return null;
  return <div className="flex items-center justify-end gap-1 border-t border-border-subtle px-3 py-1.5 text-xs text-muted-foreground">
    <span className="mr-2 tabular-nums">{page * 5 + 1}–{Math.min(page * 5 + 5, count)} of {count}</span>
    <Button variant="ghost" size="icon-sm" aria-label="Previous page" disabled={page === 0} onClick={() => onChange(page - 1)}><CaretLeft size={14} /></Button>
    <Button variant="ghost" size="icon-sm" aria-label="Next page" disabled={(page + 1) * 5 >= count} onClick={() => onChange(page + 1)}><CaretRight size={14} /></Button>
  </div>;
}

export function Include({ title, checked, disabled, onChange }: { title: string; checked: boolean; disabled?: boolean; onChange: (checked: boolean) => void }) {
  return <Checkbox className="mt-1" checked={checked} disabled={disabled} aria-label={`Include ${title}`} onCheckedChange={(value) => onChange(value === true)} />;
}

export function RowActions({ title, editing, disabled, canRemove, onEdit, onSave, onCancel, onRemove }: {
  title: string; editing: boolean; disabled: boolean; canRemove: boolean; onEdit: () => void; onSave: () => void; onCancel: () => void; onRemove: () => void;
}) {
  const action = (label: string, icon: React.ReactNode, onClick: () => void, off = disabled) => <Button variant="ghost" size="icon-xs" disabled={off} aria-label={`${label} ${title}`} onClick={onClick}>{icon}</Button>;
  return <div className="flex shrink-0 items-center">
    {editing ? <>{action("Cancel editing", <X size={14} />, onCancel)}{action("Save", <Check size={14} />, onSave)}</> : <>{action("Edit", <PencilSimple size={14} />, onEdit)}{action("Remove", <Trash size={14} />, onRemove, disabled || !canRemove)}</>}
  </div>;
}

export function ChangedSince({ fields, live }: { fields: string[]; live: Row | null | undefined }) {
  if (!fields.length) return null;
  return <Tooltip><TooltipTrigger asChild>
    <span tabIndex={0} aria-label="Changed since" className="inline-flex text-warning"><Warning size={14} /></span>
  </TooltipTrigger><TooltipContent>Changed since — {live ? fields.map((field) => `now ${field}: ${displayValue(live[field], field)}`).join("; ") : "this item no longer exists"}</TooltipContent></Tooltip>;
}

export function displayValue(value: unknown, key?: string): string {
  if (value == null || value === "") return "none";
  if (["due", "starts", "start", "end"].includes(key ?? "")) {
    const parts = /^(\d{4})-(\d\d)-(\d\d)$/.exec(String(value));
    if (parts) return new Date(Number(parts[1]), Number(parts[2]) - 1, Number(parts[3])).toLocaleDateString(displayLocale, { day: "numeric", month: "short", year: "numeric" });
    const ms = sqliteUtcToMs(String(value));
    if (ms != null) return fmtClock(ms, true);
  }
  if (Array.isArray(value)) return value.join(", ") || "none";
  return String(value);
}

export function dateShift(from: unknown, to: unknown): string | null {
  if (typeof from !== "string" || typeof to !== "string") return null;
  // Compare local calendar days, so a daylight-saving shift still reads +1.
  const day = (value: string) => /^\d{4}-\d\d-\d\d/.test(value) ? Date.parse(value.slice(0, 10) + "T00:00:00Z") : NaN;
  const days = Math.round((day(to) - day(from)) / 86_400_000);
  return Number.isFinite(days) && days !== 0 ? `${days > 0 ? "+" : ""}${days} ${Math.abs(days) === 1 ? "day" : "days"}` : null;
}
