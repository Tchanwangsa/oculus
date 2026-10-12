// Tells Rust (`shell/keys/fallback.rs`) about a ⌘-keydown this page didn't
// cancel, so the menu gets it. Every frame: an editor may live in an iframe.
(function () {
  var modifiers = { Meta: 1, Shift: 1, Alt: 1, Control: 1, CapsLock: 1 };
  window.addEventListener(
    'keydown',
    function (e) {
      if (!e.metaKey || !e.isTrusted || modifiers[e.key]) return;
      // After dispatch, when `defaultPrevented` is final.
      setTimeout(function () {
        if (e.defaultPrevented) return;
        var handlers = window.webkit && window.webkit.messageHandlers;
        var handler = handlers && handlers.oculusKeys;
        if (handler) handler.postMessage('unhandled');
      }, 0);
    },
    true
  );
})();
