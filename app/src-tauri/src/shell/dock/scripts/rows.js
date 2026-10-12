// Chat-list row watcher, main frame, Instagram/WhatsApp/Messenger. Each site
// decodes its own realtime traffic into a list of chat rows, so a row that
// turns unread carries the newest sender and preview. Read-only: it observes
// the list and changes nothing. `SERVICE` is set by `scripts.rs`.
//
// Posts `{kind: "row", name, preview}` when a known row becomes unread, when
// an unread row's preview changes, or when an unread row nobody has seen
// appears at the top. Rows are keyed by name, so a row that only moved (or
// re-mounted) posts nothing. The list mounts empty and fills over seconds,
// so nothing posts until it settles: quiet for SETTLE_QUIET ms, or
// SETTLE_MAX ms after the first rows.
const sites = {
  instagram: {
    path: /^\/direct/,
    list: '[aria-label="Thread list"]',
    // Only thread rows carry a relative-time <abbr>; the Notes tray doesn't.
    rows: (list) => [
      ...new Set(
        [...list.querySelectorAll('abbr')]
          .map((a) => a.closest('div[role="button"], a, [role="link"]'))
          .filter(Boolean),
      ),
    ],
    name: (row, lines) => lines[0] || '',
    badge: () => null,
  },
  whatsapp: {
    path: /^\//,
    list: '[role="grid"][aria-label="Chat list"]',
    rows: (list) => [...list.querySelectorAll('[role="row"]')],
    name: (row, lines) => {
      const t = row.querySelector('span[title]');
      return t ? t.getAttribute('title') || '' : lines[0] || '';
    },
    badge: (row) => {
      const b = row.querySelector('[aria-label*="unread message"]');
      return b ? b.getAttribute('aria-label') : null;
    },
  },
  messenger: {
    path: /^\/messages/,
    list: '[role="grid"][aria-label="Chats"]',
    rows: (list) => [...list.querySelectorAll('[role="row"]')],
    name: (row, lines) => {
      const b = row.querySelector('[aria-label^="More options for "]');
      return b ? b.getAttribute('aria-label').replace(/^More options for /, '') : lines[0] || '';
    },
    badge: (row) => {
      const u = row.querySelector('[aria-label*="nread"]');
      return u ? u.getAttribute('aria-label') : null;
    },
  },
};

const site = sites[SERVICE] || null;

const MAX_ROWS = 30;
const SCAN_DELAY = 600;
const SETTLE_QUIET = 2000;
const SETTLE_MAX = 15000;
// Times ("3m", "16:04", "Yesterday"), dates, badge counts and separators.
const NOISE =
  /^(\d{1,2}[:.]\d{2}( ?[AP]M)?|Yesterday|Today|Just now|Monday|Tuesday|Wednesday|Thursday|Friday|Saturday|Sunday|\d+[smhdwy]|\d{1,2}\/\d{1,2}\/\d{2,4}|\d+\+?|[·•:|]|Unread)$/;

const read = (list) =>
  site.rows(list).slice(0, MAX_ROWS).map((el) => {
    // innerText collapses to one line while the window is hidden (no
    // layout), so read leaf elements' textContent instead.
    const lines = [];
    for (const leaf of el.querySelectorAll('span, abbr')) {
      if (leaf.children.length) continue;
      const t = (leaf.textContent || '').trim();
      if (t && t !== lines[lines.length - 1]) lines.push(t);
    }
    const name = (site.name(el, lines) || '').trim();
    const time = el.querySelector('abbr');
    const ago = time ? (time.textContent || '').trim() : null;
    const badge = site.badge(el);
    // Unread rows draw their preview bold (Instagram, Messenger).
    let bold = false;
    for (const leaf of el.querySelectorAll('span')) {
      if (leaf.children.length) continue;
      const t = (leaf.textContent || '').trim();
      if (!t || t === name) continue;
      if (parseInt(getComputedStyle(leaf).fontWeight, 10) >= 600) {
        bold = true;
        break;
      }
    }
    const preview = lines
      .filter((x) => x !== name && x !== ago && x !== badge && !NOISE.test(x))
      .join(' | ')
      .slice(0, 400);
    return { name, preview, unread: !!badge || bold };
  });

// Every row ever seen, by name, for the page's life: a list that re-mounts
// or scrolls compares against this, not just the last scan.
const known = new Map();
let list = null;
let observer = null;
let scanTimer = null;
let quietTimer = null;
let maxTimer = null;
let settled = false;

const scan = () => {
  scanTimer = null;
  if (!list || !list.isConnected) return;
  const rows = read(list);
  if (!rows.length) return;
  if (!settled) {
    for (const r of rows) if (r.name) known.set(r.name, r);
    clearTimeout(quietTimer);
    quietTimer = setTimeout(settle, SETTLE_QUIET);
    if (!maxTimer) maxTimer = setTimeout(settle, SETTLE_MAX);
    return;
  }
  rows.forEach((r, index) => {
    if (!r.name) return;
    const was = known.get(r.name);
    known.set(r.name, r);
    if (!r.unread) return;
    const fresh = was ? !was.unread || was.preview !== r.preview : index === 0;
    if (fresh) post({ kind: 'row', name: r.name.slice(0, 200), preview: r.preview });
  });
};

// The baseline: one last silent scan, then every later change counts.
const settle = () => {
  if (settled) return;
  scan();
  settled = true;
  clearTimeout(quietTimer);
  clearTimeout(maxTimer);
};

// A mutation means the list isn't quiet yet; the scan it schedules restarts
// the quiet timer.
const changed = () => {
  if (!settled) clearTimeout(quietTimer);
  if (!scanTimer) scanTimer = setTimeout(scan, SCAN_DELAY);
};

const attach = () => {
  const found = site.path.test(location.pathname) ? document.querySelector(site.list) : null;
  if (!found || found === list) return;
  if (observer) observer.disconnect();
  list = found;
  observer = new MutationObserver(changed);
  observer.observe(list, { subtree: true, childList: true, characterData: true, attributes: true });
  changed();
};

if (site) {
  setInterval(attach, 2000);
  attach();
}
