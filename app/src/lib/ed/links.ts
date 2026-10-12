/** Whether `url` is an Ed page. Ed serves every region from `edstem.org` and
 *  its subdomains, so the host alone decides, never a region. */
export function isEdUrl(url: string | null | undefined): url is string {
  if (!url) return false;
  try {
    const host = new URL(url).hostname.toLowerCase();
    return host === "edstem.org" || host.endsWith(".edstem.org");
  } catch {
    return false;
  }
}

/** The Ed thread id in a thread's web URL (`…/discussion/<id>`), else null. */
export function edThreadId(url: string | null | undefined): number | null {
  if (!isEdUrl(url)) return null;
  const id = /\/discussion\/(\d+)\/?$/.exec(new URL(url).pathname)?.[1];
  return id ? Number(id) : null;
}
