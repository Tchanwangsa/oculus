import type { ReactNode } from 'react';
import { DropdownMenu } from 'radix-ui';
import { ArrowLineDownIcon, ArrowLineUpIcon, CaretRightIcon, CheckIcon, CopyIcon, DotsThreeIcon, FrameCornersIcon, GridFourIcon, StackIcon, TrashIcon } from '@phosphor-icons/react';
import { A4, PAPER_TEMPLATES, templateOf, type PaperPage, type PaperTemplate } from '@/lib/canvas/paper';
import type { PageSize } from '@/lib/canvas/pages';
import { cn } from '@/lib/utils';
import { PaperThumb, TEMPLATE_LABELS } from './PaperThumb';

/** US Letter in points (8.5 x 11 in). */
const LETTER = { width: 612, height: 792 } as const;
const SIZES: { label: string; size: PageSize }[] = [
  { label: 'A4 portrait', size: A4 },
  { label: 'A4 landscape', size: { width: A4.height, height: A4.width } },
  { label: 'Letter portrait', size: LETTER },
  { label: 'Letter landscape', size: { width: LETTER.height, height: LETTER.width } },
];
const sizeKey = ({ width, height }: PageSize) => `${width}x${height}`;

export interface PageMenuActions {
  insert: (side: 'before' | 'after') => void;
  duplicate: () => void;
  setTemplate: (template: PaperTemplate) => void;
  applyTemplateToAll: () => void;
  setSize: (size: PageSize) => void;
  remove: () => void;
}

const ITEM = 'flex cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-[12px] outline-none data-disabled:pointer-events-none data-disabled:opacity-50 data-highlighted:bg-accent data-[state=open]:bg-accent';
export const CONTENT = 'z-50 min-w-48 rounded-xl border border-border-subtle bg-popover p-1 text-popover-foreground shadow-lg outline-none';
const SEPARATOR = <DropdownMenu.Separator className="mx-1 my-1 h-px bg-border-subtle" />;

export function Item({ icon, children, onSelect, disabled, destructive }: { icon: ReactNode; children: ReactNode; onSelect: () => void; disabled?: boolean; destructive?: boolean }) {
  return <DropdownMenu.Item onSelect={onSelect} disabled={disabled} className={cn(ITEM, destructive && 'text-destructive data-highlighted:bg-destructive/10')}>
    <span className={cn('flex size-4 items-center justify-center', destructive ? 'text-destructive' : 'text-muted-foreground')}>{icon}</span>{children}
  </DropdownMenu.Item>;
}

function Choice({ value, children }: { value: string; children: ReactNode }) {
  return <DropdownMenu.RadioItem value={value} className={ITEM}>
    {children}
    <DropdownMenu.ItemIndicator className="ml-auto text-brand"><CheckIcon size={13} weight="bold" /></DropdownMenu.ItemIndicator>
  </DropdownMenu.RadioItem>;
}

/** A page's ⋯ menu: pages beside it, a copy, its paper and size, and removing it. */
export function PageMenu({ page, number, canRemove, actions }: { page: PaperPage; number: number; canRemove: boolean; actions: PageMenuActions }) {
  const orientation = page.width > page.height ? 'landscape' : 'portrait';
  return <DropdownMenu.Root>
    <DropdownMenu.Trigger asChild>
      <button type="button" aria-label={`Page ${number} options`}
        className="flex size-7 cursor-pointer items-center justify-center rounded-full border border-border-subtle bg-card/95 text-muted-foreground shadow-xs outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring data-[state=open]:text-foreground">
        <DotsThreeIcon size={16} weight="bold" />
      </button>
    </DropdownMenu.Trigger>
    <DropdownMenu.Portal>
      <DropdownMenu.Content side="right" align="start" sideOffset={6} collisionPadding={12} className={CONTENT} onCloseAutoFocus={event => event.preventDefault()}>
        <Item icon={<ArrowLineUpIcon size={15} />} onSelect={() => actions.insert('before')}>Add page before</Item>
        <Item icon={<ArrowLineDownIcon size={15} />} onSelect={() => actions.insert('after')}>Add page after</Item>
        <Item icon={<CopyIcon size={15} />} onSelect={actions.duplicate}>Duplicate</Item>
        {SEPARATOR}
        <DropdownMenu.Sub>
          <DropdownMenu.SubTrigger className={ITEM}>
            <span className="flex size-4 items-center justify-center text-muted-foreground"><GridFourIcon size={15} /></span>Template
            <CaretRightIcon size={12} className="ml-auto text-muted-foreground" />
          </DropdownMenu.SubTrigger>
          <DropdownMenu.Portal>
            <DropdownMenu.SubContent sideOffset={4} collisionPadding={12} className={CONTENT}>
              <DropdownMenu.RadioGroup value={templateOf(page)} onValueChange={value => actions.setTemplate(value as PaperTemplate)}>
                {PAPER_TEMPLATES.map(template => <Choice key={template} value={template}>
                  <PaperThumb template={template} orientation={orientation} long={26} />{TEMPLATE_LABELS[template]}
                </Choice>)}
              </DropdownMenu.RadioGroup>
              {SEPARATOR}
              <Item icon={<StackIcon size={15} />} onSelect={actions.applyTemplateToAll}>Apply to every page</Item>
            </DropdownMenu.SubContent>
          </DropdownMenu.Portal>
        </DropdownMenu.Sub>
        <DropdownMenu.Sub>
          <DropdownMenu.SubTrigger className={ITEM}>
            <span className="flex size-4 items-center justify-center text-muted-foreground"><FrameCornersIcon size={15} /></span>Size
            <CaretRightIcon size={12} className="ml-auto text-muted-foreground" />
          </DropdownMenu.SubTrigger>
          <DropdownMenu.Portal>
            <DropdownMenu.SubContent sideOffset={4} collisionPadding={12} className={CONTENT}>
              <DropdownMenu.RadioGroup value={sizeKey(page)} onValueChange={value => { const choice = SIZES.find(item => sizeKey(item.size) === value); if (choice) actions.setSize(choice.size); }}>
                {SIZES.map(({ label, size }) => <Choice key={label} value={sizeKey(size)}>{label}</Choice>)}
              </DropdownMenu.RadioGroup>
            </DropdownMenu.SubContent>
          </DropdownMenu.Portal>
        </DropdownMenu.Sub>
        {SEPARATOR}
        <Item icon={<TrashIcon size={15} />} onSelect={actions.remove} disabled={!canRemove} destructive>Delete page</Item>
      </DropdownMenu.Content>
    </DropdownMenu.Portal>
  </DropdownMenu.Root>;
}
