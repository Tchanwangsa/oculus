import type { ReactNode } from 'react';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { cn } from '@/lib/utils';

export interface SegmentOption<T> { value: T; label: string; content: ReactNode }

/** One choice from a few: a grey track with the chosen segment raised as a card in the brand colour. */
export function Segmented<T extends string | number>({ label, options, value, onChange, className }: {
  label: string; options: SegmentOption<T>[]; value: T | undefined; onChange: (value: T) => void; className?: string;
}) {
  return <div role="group" aria-label={label} className={cn('flex min-w-0 gap-0.5 rounded-[10px] bg-muted p-0.5', className)}>
    {options.map(option => {
      const active = option.value === value;
      return <Tooltip key={String(option.value)}>
        <TooltipTrigger asChild>
          <button type="button" aria-label={option.label} aria-pressed={active} onClick={() => onChange(option.value)}
            className={cn('flex h-7 min-w-0 flex-1 cursor-pointer items-center justify-center rounded-lg text-muted-foreground transition-[color,background-color,box-shadow] outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-brand',
              active && 'bg-card text-brand shadow-xs hover:text-brand')}>
            {option.content}
          </button>
        </TooltipTrigger>
        <TooltipContent side="bottom" sideOffset={6}>{option.label}</TooltipContent>
      </Tooltip>;
    })}
  </div>;
}
