// The one-hop matcher, the JS twin of `rules.rs`, used by the guard and the
// leave-on-park script and tested by `app/tests/lib/dock/hop.test.ts`.
// `RULES` is the service's `rules/<id>.json`, prepended by `scripts.rs`.
// No DOM here, so the test can load it as is.
const hop = (() => {
  const match = (template, segments) => {
    if (segments.length < template.length) return null;
    let key = '';
    for (let i = 0; i < template.length; i++) {
      const want = template[i];
      if (want === ':key') key = segments[i];
      else if (want !== ':_' && want !== segments[i]) return null;
    }
    return key;
  };
  const hit = (templates, segments) => templates.some((t) => match(t, segments) !== null);

  // `{kind: 'home'|'pass'|'item'|'blocked'|'offsite', id?}`.
  const classify = (href, base) => {
    let url;
    try {
      url = new URL(href, base);
    } catch (e) {
      return { kind: 'blocked' };
    }
    if (url.protocol !== 'http:' && url.protocol !== 'https:') return { kind: 'offsite' };
    if (!RULES.hosts.includes(url.hostname.toLowerCase())) return { kind: 'offsite' };
    const segments = url.pathname.split('/').filter(Boolean);
    if (hit(RULES.home, segments)) return { kind: 'home' };
    if (hit(RULES.pass, segments)) return { kind: 'pass' };
    for (const rule of RULES.items) {
      const key = match(rule.path, segments);
      if (key !== null) return { kind: 'item', id: rule.kind + ':' + key };
    }
    return { kind: 'blocked' };
  };

  // `state` is null in the messages, else the open item's id.
  const allows = (state, place) => {
    if (place.kind === 'item') return state === null || state === place.id;
    return place.kind !== 'blocked';
  };

  // The hop state after arriving at an allowed `place`.
  const next = (state, place) => {
    if (place.kind === 'home') return null;
    if (place.kind === 'item') return place.id;
    return state;
  };

  return { classify, allows, next };
})();
