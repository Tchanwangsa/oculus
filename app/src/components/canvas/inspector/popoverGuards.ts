import type { KeyboardEvent } from 'react';

const writing = () => document.activeElement instanceof HTMLTextAreaElement;

/**
 * Props for a popover opened from the inspector. It focuses itself rather than
 * its first option, whose ring and tooltip would read as the current choice. A
 * label or text being written keeps focus instead, so a pick restyles it rather
 * than committing it. Keys pressed inside stay out of the canvas's shortcuts.
 */
export const inspectorPopoverProps = {
  onOpenAutoFocus: (event: Event) => {
    event.preventDefault();
    if (!writing()) (event.target as HTMLElement).focus();
  },
  onCloseAutoFocus: (event: Event) => { if (writing()) event.preventDefault(); },
  onKeyDown: (event: KeyboardEvent) => event.stopPropagation(),
};
