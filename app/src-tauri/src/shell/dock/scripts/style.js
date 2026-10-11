// Adds the service's trim sheet (`CSS`, prepended by `scripts.rs` from
// `instagram.css` or `facebook.css`) as a <style> at document start, before
// the nav can draw. `<head>` may not exist yet, so it goes on the root
// element, and is put back if the page ever removes it or swaps the root.
const style = document.createElement('style');
style.textContent = CSS;
const watch = new MutationObserver(() => attach());
let watched = null;
const attach = () => {
  try {
    const root = document.documentElement;
    if (!root) return;
    if (style.parentNode !== root) root.appendChild(style);
    if (watched !== root) {
      watched = root;
      watch.observe(root, { childList: true });
    }
  } catch (e) {}
};
try {
  watch.observe(document, { childList: true });
} catch (e) {}
attach();
