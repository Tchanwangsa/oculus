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
