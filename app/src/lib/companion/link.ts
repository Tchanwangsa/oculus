/**
 * A share link is `http://<ip>:<port>/c/#<shareId>.<secret>`. The fragment never
 * reaches the server or a Referer; the page takes it, strips it from the address
 * bar at once, and keeps it in memory plus this tab's sessionStorage so a reload
 * of the same tab still works. The secret then goes only into the Authorization
 * header and the socket's hello frame.
 */
export interface ShareCredentials { shareId: string; secret: string }

const SHARE_ID = /^[0-9a-f]{32}$/;
const SECRET = /^[A-Za-z0-9_-]{16,128}$/;
const SESSION_KEY = "oculus:companion:link";

export function parseShareFragment(hash: string): ShareCredentials | null {
  const value = hash.startsWith("#") ? hash.slice(1) : hash;
  const dot = value.indexOf(".");
  if (dot < 0) return null;
  const shareId = value.slice(0, dot), secret = value.slice(dot + 1);
  return SHARE_ID.test(shareId) && SECRET.test(secret) ? { shareId, secret } : null;
}

export interface LinkPage {
  location: Pick<Location, "hash" | "pathname" | "search">;
  history: Pick<History, "replaceState" | "state">;
  session: Pick<Storage, "getItem" | "setItem" | "removeItem"> | null;
}

/** The link's credentials, from the fragment (which is then removed) or from an earlier load of this tab. */
export function takeShareCredentials(page: LinkPage): ShareCredentials | null {
  const { location, history, session } = page;
  if (location.hash) {
    const fromHash = parseShareFragment(location.hash);
    // Strip any fragment, valid or not: it may still hold a secret.
    history.replaceState(history.state, "", `${location.pathname}${location.search}`);
    if (fromHash) {
      try { session?.setItem(SESSION_KEY, `${fromHash.shareId}.${fromHash.secret}`); } catch { /* memory only */ }
      return fromHash;
    }
  }
  try { return parseShareFragment(session?.getItem(SESSION_KEY) ?? ""); } catch { return null; }
}

function sessionOrNull(): Storage | null {
  try { return window.sessionStorage; } catch { return null; }
}

export function takeBrowserShareCredentials(): ShareCredentials | null {
  return takeShareCredentials({ location: window.location, history: window.history, session: sessionOrNull() });
}

/** Redacts the secret from text that is about to be shown or logged. */
export function redactSecret(text: string, credentials: ShareCredentials): string {
  return text.split(credentials.secret).join("•••");
}
