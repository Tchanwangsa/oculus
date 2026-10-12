// Shared head of every dock init script (`scripts.rs` wraps it and the script
// in one function). `post` hands a plain object of strings to the app's `dock`
// WKScriptMessageHandler. The handler is added just after the page is
// created, so the first document can run before it exists: messages queue
// (a few, never unbounded) and retry until it does.
const post = (() => {
  const queue = [];
  const wait = window.setTimeout.bind(window);
  let retrying = false;
  const handler = () => {
    try {
      return window.webkit.messageHandlers.dock || null;
    } catch (e) {
      return null;
    }
  };
  const retry = () => {
    retrying = false;
    flush();
  };
  const flush = () => {
    const h = handler();
    if (!h) {
      if (queue.length && !retrying) {
        retrying = true;
        wait(retry, 250);
      }
      return;
    }
    while (queue.length) {
      try {
        h.postMessage(queue.shift());
      } catch (e) {}
    }
  };
  return (message) => {
    if (queue.length < 20) queue.push(message);
    flush();
  };
})();
const text = (value, max) => {
  try {
    return (value == null ? '' : String(value)).slice(0, max);
  } catch (e) {
    return '';
  }
};
