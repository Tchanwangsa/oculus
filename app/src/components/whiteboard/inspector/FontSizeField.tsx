import { useState } from 'react';
import { Input } from '@/components/ui/input';

const MIN = 4, MAX = 400;

/** Plain text so a half-typed size never commits; Enter or leaving the field applies it, Escape reverts. */
export function FontSizeField({ value, onChange }: { value: number; onChange: (size: number) => void }) {
  const [draft, setDraft] = useState<string | null>(null);
  const commit = () => {
    if (draft === null) return;
    setDraft(null);
    const size = Number.parseFloat(draft);
    if (Number.isFinite(size) && size > 0) onChange(Math.min(MAX, Math.max(MIN, Math.round(size))));
  };
  return <Input aria-label="Font size" inputMode="numeric" value={draft ?? String(Math.round(value))}
    onChange={event => setDraft(event.target.value)} onFocus={event => event.currentTarget.select()} onBlur={commit}
    onKeyDown={event => {
      if (event.key === 'Enter') { event.preventDefault(); commit(); }
      else if (event.key === 'Escape') { event.preventDefault(); setDraft(null); event.currentTarget.blur(); }
    }}
    className="h-8 w-12 shrink-0 rounded-[10px] px-1.5 text-center text-[12px] tabular-nums shadow-none" />;
}
