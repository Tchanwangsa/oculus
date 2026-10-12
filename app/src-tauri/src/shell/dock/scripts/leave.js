// Run on a page as it goes from shown to parked (`page.rs`). The service
// marks an open thread read even while it is parked, so the page goes back
// to `PARK_PATH` (its inbox) the way the router itself navigates: a push,
// which the guard allows (home), and a popstate the router follows. Login
// pages are left alone.
try {
  const place = hop.classify(location.href, location.href);
  if (location.pathname !== PARK_PATH && (place.kind === 'home' || place.kind === 'item')) {
    history.pushState(null, '', PARK_PATH);
    window.dispatchEvent(new PopStateEvent('popstate', { state: null }));
  }
} catch (e) {}
