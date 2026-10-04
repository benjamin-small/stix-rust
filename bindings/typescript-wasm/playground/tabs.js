// Index of the tab to activate for a keydown on the tablist, or null when the
// key is not a tab-navigation key. ArrowLeft/ArrowRight wrap.
export function nextTabIndex(current, key, count) {
  switch (key) {
    case "ArrowRight": return (current + 1) % count;
    case "ArrowLeft": return (current - 1 + count) % count;
    case "Home": return 0;
    case "End": return count - 1;
    default: return null;
  }
}
