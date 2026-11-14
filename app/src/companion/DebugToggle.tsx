import { useEffect, useState } from 'react';
import { BugIcon } from '@phosphor-icons/react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import { showGestureOverlay } from './gestureOverlay';

const STORAGE_KEY = 'oculus-companion-gesture-overlay';
const load = () => { try { return localStorage.getItem(STORAGE_KEY) === '1'; } catch { return false; } };
const save = (on: boolean) => { try { localStorage.setItem(STORAGE_KEY, on ? '1' : '0'); } catch { /* storage unavailable */ } };

/** Shows or hides the gesture overlay (`gestureOverlay.ts`) for screen-recording a problem; remembered on this device. */
export function DebugToggle() {
  const [on, setOn] = useState(load);
  useEffect(() => (on ? showGestureOverlay() : undefined), [on]);
  const label = on ? 'Hide touch and gesture overlay' : 'Show touch and gesture overlay';
  return <Button type="button" variant="outline" size="icon" aria-label={label} aria-pressed={on} title={label}
    className={cn('canvas-debug absolute top-3 right-14 z-40 bg-card shadow-xs', on && 'border-brand bg-brand text-white hover:bg-brand hover:text-white')}
    onClick={() => { const next = !on; setOn(next); save(next); }}>
    <BugIcon size={17} weight={on ? 'fill' : 'regular'} />
  </Button>;
}
