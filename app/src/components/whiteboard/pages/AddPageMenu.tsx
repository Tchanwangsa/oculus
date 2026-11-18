import { DropdownMenu } from 'radix-ui';
import { FilePdfIcon, FilePlusIcon, PlusIcon } from '@phosphor-icons/react';
import { CONTENT, Item } from './PageMenu';
import { THUMB_WIDTH } from './usePageThumbnails';

const BUTTON = 'mx-auto mt-2 flex h-9 cursor-pointer items-center justify-center gap-1.5 rounded-lg border border-dashed border-border text-[12px] text-muted-foreground outline-none hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring data-[state=open]:bg-accent data-[state=open]:text-foreground';

/**
 * The sidebar's + at the end of the pages. A new page takes the last page's paper and size; where the host has a library
 * (`onPdf`), it opens a choice between that and PDF pages, otherwise it adds the page straight away.
 */
export function AddPageMenu({ onAdd, onPdf }: { onAdd: () => void; onPdf?: () => void }) {
  const label = <><PlusIcon size={14} />Add page</>;
  if (!onPdf) return <button type="button" onClick={onAdd} style={{ width: THUMB_WIDTH }} className={BUTTON}>{label}</button>;
  return <DropdownMenu.Root>
    <DropdownMenu.Trigger asChild><button type="button" style={{ width: THUMB_WIDTH }} className={BUTTON}>{label}</button></DropdownMenu.Trigger>
    <DropdownMenu.Portal>
      <DropdownMenu.Content side="top" align="center" sideOffset={6} collisionPadding={12} className={CONTENT} onCloseAutoFocus={event => event.preventDefault()}>
        <Item icon={<FilePlusIcon size={15} />} onSelect={onAdd}>New page</Item>
        <Item icon={<FilePdfIcon size={15} />} onSelect={onPdf}>Pages from a PDF…</Item>
      </DropdownMenu.Content>
    </DropdownMenu.Portal>
  </DropdownMenu.Root>;
}
