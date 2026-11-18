import { ArrowBendUpLeftIcon, ScissorsIcon, ScribbleLoopIcon, type Icon } from '@phosphor-icons/react';
import type { EraserMode, EraserSize } from '@/lib/canvas/erase';
import type { CanvasToolPreferences } from '@/lib/canvas/toolPreferences';
import { CanvasIconButton } from '../CanvasToolbar';
import { Segmented } from './Segmented';
import { StyleGroup as Group } from './StyleGroup';

const MODES: [EraserMode, string, Icon][] = [['stroke', 'Whole stroke', ScribbleLoopIcon], ['partial', 'Partial', ScissorsIcon]];
/** Dot diameters in the picker: the three radii (6, 12 and 24 px) scaled to fit a segment. */
const SIZES: [EraserSize, string, number][] = [['small', 'Small', 5], ['medium', 'Medium', 9], ['large', 'Large', 15]];

/** The eraser's settings in the style bar: what it erases, its size when it cuts, and whether lifting it returns to the last tool. */
export function EraserControls({ preferences, onChange }: { preferences: CanvasToolPreferences; onChange: (patch: Partial<CanvasToolPreferences>) => void }) {
  return <>
    <Group>
      <Segmented label="Eraser mode" className="w-20 shrink-0" value={preferences.eraserMode} onChange={eraserMode => onChange({ eraserMode })}
        options={MODES.map(([value, label, Glyph]) => ({ value, label, content: <Glyph size={16} /> }))} />
    </Group>
    {preferences.eraserMode === 'partial' ? <Group>
      <Segmented label="Eraser size" className="w-28 shrink-0" value={preferences.eraserSize} onChange={eraserSize => onChange({ eraserSize })}
        options={SIZES.map(([value, label, size]) => ({ value, label, content: <span className="rounded-full bg-current" style={{ width: size, height: size }} /> }))} />
    </Group> : null}
    <Group>
      <CanvasIconButton label="Return to the previous tool after erasing" active={preferences.eraserAutoReturn} aria-pressed={preferences.eraserAutoReturn} className="size-8"
        onClick={() => onChange({ eraserAutoReturn: !preferences.eraserAutoReturn })}><ArrowBendUpLeftIcon size={17} /></CanvasIconButton>
    </Group>
  </>;
}
