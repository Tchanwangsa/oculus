import { useEffect, useState } from "react";
import type { Icon as PhosphorIcon } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";

let catalogue: Map<string, PhosphorIcon> | null = null;
let loading: Promise<Map<string, PhosphorIcon>> | null = null;

export function loadIconCatalogue(): Promise<Map<string, PhosphorIcon>> {
  loading ??= import("./iconCatalog").then(({ ICON_BY_NAME }) => {
    catalogue = ICON_BY_NAME;
    return ICON_BY_NAME;
  }).catch(error => { loading = null; throw error; });
  return loading;
}

/** A fixed icon slot; only custom glyphs need the lazy catalogue. */
export function CatalogueIcon({ name, fallback, color, size = 14, weight = "fill", className }: {
  name?: string;
  fallback: PhosphorIcon;
  color: string;
  size?: number;
  weight?: "fill" | "regular";
  className?: string;
}) {
  const [, setLoaded] = useState(catalogue != null);
  useEffect(() => {
    if (!name || catalogue) return;
    let live = true;
    loadIconCatalogue().then(() => live && setLoaded(true)).catch(() => {});
    return () => { live = false; };
  }, [name]);
  const Icon = (name && catalogue?.get(name)) || fallback;
  return <span className={cn("flex shrink-0 items-center justify-center", className)} style={{ width: size, height: size }}>
    <Icon size={size} weight={weight} color={color} />
  </span>;
}
