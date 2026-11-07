import { PaintBucketIcon } from '@phosphor-icons/react';
import { ColorPopover } from '@/components/ui/color/ColorField';
import { swatchBackground } from '@/components/ui/color/ColorPalette';
import type { SectionProps } from './InspectorSection';
import { FILL_PALETTE } from './palette';
import { inspectorPopoverProps } from './popoverGuards';

/** The fill as one button, a bucket beside its colour, that opens the fill palette below the bar. */
export function FillButton({ values, onChange }: SectionProps) {
  return <ColorPopover value={values.fill} onChange={fill => onChange({ fill })} label="Fill" tooltip="Fill" palette={FILL_PALETTE} opaque side="bottom" align="center" contentProps={inspectorPopoverProps}
    className="flex h-8 items-center gap-1 rounded-lg px-1.5 text-muted-foreground outline-none hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-brand">
    <PaintBucketIcon size={17} />
    <span className="block size-5 rounded-md border border-black/10" style={{ background: swatchBackground(values.fill) }} />
  </ColorPopover>;
}
