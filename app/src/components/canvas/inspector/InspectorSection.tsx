import type { ReactNode } from 'react';
import type { StylePatch, StyleValues } from '@/lib/canvas/style';

export interface SectionProps { values: StyleValues; onChange: (patch: StylePatch) => void }

export function InspectorSection({ label, aside, children }: { label: string; aside?: ReactNode; children: ReactNode }) {
  return <div className="space-y-2">
    <div className="flex items-center justify-between gap-2 text-muted-foreground"><span>{label}</span>{aside}</div>
    {children}
  </div>;
}
