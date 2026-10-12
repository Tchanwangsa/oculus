// One-hop guard, main frame, services with a rule table (Instagram). Keeps
// the page on its messages plus one shared item (`hop.js`, docs/dock.md
// "A page stays on its messages"). Unlike the other scripts it changes what
// the page does: blocked clicks do nothing and blocked route changes are
// undone. It posts nothing and never throws into the page.

// The page's hop state: null in the messages, else the open item's id.
let state = null;
let lastUrl = location.href;
let lastState = null;
try {
  lastState = history.state;
  state = hop.next(null, hop.classify(location.href, location.href));
} catch (e) {}

// Set while the guard undoes a replace or pop, so route changes the router
// makes in response are not judged again.
let undoing = 0;
const pushState = history.pushState;
const replaceState = history.replaceState;

const accept = (place) => {
  state = hop.next(state, place);
  lastUrl = location.href;
  lastState = history.state;
};

// A push is undone with back(), which also returns the view (its popstate is
// judged like any other and lands on the last allowed URL). A replace or pop
// has no entry to go back over, so the last allowed URL is put back and the
// router follows a synthetic popstate, which the listener below ignores.
const undo = (how) => {
  if (how === 'push') {
    history.back();
    return;
  }
  undoing++;
  try {
    replaceState.call(history, lastState, '', lastUrl);
    window.dispatchEvent(new PopStateEvent('popstate', { state: lastState }));
  } finally {
    undoing--;
  }
};

const judge = (how) => {
  try {
    const place = hop.classify(location.href, location.href);
    if (hop.allows(state, place)) accept(place);
    else undo(how);
  } catch (e) {}
};

for (const [name, original, how] of [
  ['pushState', pushState, 'push'],
  ['replaceState', replaceState, 'replace'],
]) {
  history[name] = function () {
    const result = original.apply(this, arguments);
    if (!undoing) judge(how);
    return result;
  };
}

window.addEventListener(
  'popstate',
  (e) => {
    if (e.isTrusted && !undoing) judge('pop');
  },
  true,
);

// Shared items open from href-less buttons, so this sees only real links
// (profiles, the nav, links a friend sent). Off-site links that would replace
// the page open in the default browser instead (`on_new_window`).
const onClick = (e) => {
  try {
    if (e.type === 'auxclick' && e.button !== 1) return;
    const a = e
      .composedPath()
      .find((n) => n && n.tagName === 'A' && n.hasAttribute && n.hasAttribute('href'));
    if (!a) return;
    const raw = a.getAttribute('href').trim();
    if (!raw || raw.startsWith('#') || /^javascript:/i.test(raw)) return;
    const place = hop.classify(a.href, location.href);
    if (place.kind === 'offsite') {
      if (!/^https?:/i.test(a.href) || a.target === '_blank') return;
      e.preventDefault();
      e.stopImmediatePropagation();
      window.open(a.href, '_blank', 'noopener');
      return;
    }
    if (!hop.allows(state, place)) {
      e.preventDefault();
      e.stopImmediatePropagation();
    }
  } catch (err) {}
};
window.addEventListener('click', onClick, true);
window.addEventListener('auxclick', onClick, true);
