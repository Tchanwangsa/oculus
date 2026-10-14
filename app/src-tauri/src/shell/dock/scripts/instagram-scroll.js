// The list wrapper includes the header and Notes tray. Start at a thread's
// timestamp and mark its vertical scroller; changing every overflow box would
// add gutters to the chat composer and the horizontal Notes tray as well.
let scroller = null;
let pending = false;
const update = () => {
  pending = false;
  const list = document.querySelector('[aria-label="Thread list"]');
  let found = list?.querySelector('abbr') || null;
  while (found && !/^(auto|scroll|overlay)$/.test(getComputedStyle(found).overflowY)) {
    found = found.parentElement;
  }
  if (found === scroller) return;
  if (scroller) scroller.removeAttribute('data-oculus-thread-scroll');
  scroller = found;
  if (scroller) scroller.setAttribute('data-oculus-thread-scroll', '');
};
const schedule = () => {
  if (pending) return;
  pending = true;
  setTimeout(update, 100);
};
new MutationObserver(schedule).observe(document, { childList: true, subtree: true });
window.addEventListener('resize', schedule);
schedule();
