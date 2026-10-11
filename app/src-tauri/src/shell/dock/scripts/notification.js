// Notification shim, main frame, document start, every service. A
// non-browser WKWebView shows no web notifications and denies permission, so
// sites never raise one. This reports permission as granted (so the site's
// own notification setting can be turned on), and each Notification the site
// constructs is posted as `{kind: "notification", title, body, tag}` and
// otherwise behaves like one to the page. Nothing is shown and nothing on
// the page changes.
// Posted strings are cut here too; Rust clamps them again.
const MAX = 1000;
const ALL = Infinity;

class DockNotification extends EventTarget {
  #closed = false;
  constructor(title, options) {
    super();
    if (arguments.length === 0) throw new TypeError('Notification: title is required');
    const o = options && typeof options === 'object' ? options : {};
    this.title = text(title, ALL);
    this.body = text(o.body, ALL);
    this.tag = text(o.tag, ALL);
    this.icon = text(o.icon, ALL);
    this.badge = text(o.badge, ALL);
    this.image = text(o.image, ALL);
    this.lang = text(o.lang, ALL);
    this.dir = o.dir === 'ltr' || o.dir === 'rtl' ? o.dir : 'auto';
    this.data = o.data === undefined ? null : o.data;
    this.silent = !!o.silent;
    this.requireInteraction = !!o.requireInteraction;
    this.renotify = !!o.renotify;
    this.timestamp = Date.now();
    this.actions = [];
    this.vibrate = [];
    this.onclick = this.onshow = this.onclose = this.onerror = null;
    for (const type of ['click', 'show', 'close', 'error']) {
      this.addEventListener(type, (event) => {
        const handler = this['on' + type];
        if (typeof handler === 'function') handler.call(this, event);
      });
    }
    post({
      kind: 'notification',
      title: this.title.slice(0, MAX),
      body: this.body.slice(0, MAX),
      tag: this.tag.slice(0, MAX),
    });
    Promise.resolve().then(() => this.dispatchEvent(new Event('show')));
  }
  close() {
    if (this.#closed) return;
    this.#closed = true;
    this.dispatchEvent(new Event('close'));
  }
  static get permission() {
    return 'granted';
  }
  static get maxActions() {
    return 0;
  }
  static requestPermission(callback) {
    if (typeof callback === 'function') {
      try {
        callback('granted');
      } catch (e) {}
    }
    return Promise.resolve('granted');
  }
}

try {
  Object.defineProperty(window, 'Notification', {
    value: DockNotification,
    writable: true,
    configurable: true,
  });
} catch (e) {
  try {
    window.Notification = DockNotification;
  } catch (e2) {}
}

// Sites also ask the Permissions API before constructing a Notification.
try {
  const permissions = navigator.permissions;
  if (permissions && typeof permissions.query === 'function') {
    const query = permissions.query.bind(permissions);
    permissions.query = (descriptor) => {
      if (descriptor && descriptor.name === 'notifications') {
        const status = new EventTarget();
        status.name = 'notifications';
        status.state = 'granted';
        status.onchange = null;
        return Promise.resolve(status);
      }
      return query(descriptor);
    };
  }
} catch (e) {}

// A site that notifies through its service worker registration is reported
// the same way; the real call still runs (and fails quietly, as before).
try {
  const proto = window.ServiceWorkerRegistration && ServiceWorkerRegistration.prototype;
  if (proto && typeof proto.showNotification === 'function') {
    const show = proto.showNotification;
    proto.showNotification = function (title, options) {
      const o = options && typeof options === 'object' ? options : {};
      post({ kind: 'notification', title: text(title, MAX), body: text(o.body, MAX), tag: text(o.tag, MAX) });
      try {
        return Promise.resolve(show.apply(this, arguments)).catch(() => undefined);
      } catch (e) {
        return Promise.resolve();
      }
    };
  }
} catch (e) {}
