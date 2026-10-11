import { isWebUrl } from "@/lib/browser";

// History is shown on screen, so credentials (e.g. Echo360's signed playback
// URLs) are stripped before the first write.

/** Query parameters that carry a credential, whatever the site calls it. */
const SECRET_PARAMS =
  /^(x-amz-.*|access[_-]?token|id[_-]?token|refresh[_-]?token|oauth[_-]?token|token|auth|authorization|api[_-]?key|apikey|key|secret|signature|sig|hmac|policy|credential|expires|session|sessionid|sid|jwt|password|passwd|pwd|code|state|ticket|saml.*|sso.*)$/i;

/** A long unbroken value — an unnamed signature or token blob. */
const OPAQUE_VALUE = /^[A-Za-z0-9._~-]{60,}$/;

/** The URL as it should be remembered, or `null`. Drops the fragment, and
 *  the **whole** query if any part looks like a credential — a signed URL
 *  missing one parameter is neither safe nor useful. */
export function historyUrl(raw: string): string | null {
  if (!isWebUrl(raw)) return null;
  let url: URL;
  try {
    url = new URL(raw);
  } catch {
    return null;
  }
  url.hash = "";
  for (const [name, value] of url.searchParams) {
    if (SECRET_PARAMS.test(name) || OPAQUE_VALUE.test(value)) {
      url.search = "";
      break;
    }
  }
  return url.toString();
}
