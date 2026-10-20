import { useMemo } from "react";
import { encodeQr, qrPath } from "@/lib/qr";

/** The link as a QR code, drawn here (`lib/qr`), never by a remote service. It
 * stays black on white in every theme: phone cameras read inverted codes badly. */
export function ShareQr({ link, size = 168 }: { link: string; size?: number }) {
  const path = useMemo(() => {
    try { return qrPath(encodeQr(link).modules); } catch { return null; }
  }, [link]);
  if (!path) return <p className="text-xs text-muted-foreground">This link is too long for a QR code; copy it instead.</p>;
  return (
    <svg
      role="img" aria-label="QR code of the share link" width={size} height={size}
      viewBox={`0 0 ${path.extent} ${path.extent}`} shapeRendering="crispEdges"
      className="shrink-0 rounded-lg border border-border-subtle bg-white"
    >
      <path d={path.d} fill="#000" />
    </svg>
  );
}
