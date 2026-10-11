export function Swatch({ color }: { color: string }) {
  return <span className="size-2.5 shrink-0 rounded-[2px]" style={{ background: color }} />;
}
