import { getDb } from "@/lib/db";

/** host → `data:` URL, read at startup so the strip has icons early. */
export async function loadFavicons(): Promise<Record<string, string>> {
  const db = await getDb();
  const rows = await db.select<{ host: string; icon: string }[]>(
    `SELECT host, icon FROM browser_favicons`,
  );
  return Object.fromEntries(rows.map((r) => [r.host, r.icon]));
}

export async function saveFavicon(host: string, icon: string): Promise<void> {
  const db = await getDb();
  await db.execute(
    `INSERT INTO browser_favicons (host, icon) VALUES ($1, $2)
     ON CONFLICT(host) DO UPDATE SET icon = excluded.icon, updated_at = datetime('now')`,
    [host, icon],
  );
}
