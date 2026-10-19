import { useEffect } from "react";

const MARGIN = 8;

/** Keeps every open `.popover` (menus, pickers) inside the window: on a short window it scrolls inside itself, or
 *  moves up, instead of running past the bottom edge; sideways it is moved back in. */
function fit(el: HTMLElement) {
  el.style.maxHeight = "";
  el.style.overflowY = "";
  el.style.translate = "";
  const r = el.getBoundingClientRect();
  if (r.width === 0 && r.height === 0) return;
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  let dx = 0;
  let dy = 0;
  if (r.right > vw - MARGIN) dx = Math.max(vw - MARGIN - r.right, MARGIN - r.left);
  else if (r.left < MARGIN) dx = MARGIN - r.left;
  const space = vh - MARGIN - r.top;
  if (r.height > space) {
    if (space >= 260) {
      el.style.maxHeight = `${space}px`;
      el.style.overflowY = "auto";
    } else {
      const h = Math.min(r.height, vh - 2 * MARGIN);
      if (h < r.height) {
        el.style.maxHeight = `${h}px`;
        el.style.overflowY = "auto";
      }
      dy = Math.max(MARGIN, vh - MARGIN - h) - r.top;
    }
  }
  if (dx || dy) el.style.translate = `${dx}px ${dy}px`;
}

export function useFitPopovers() {
  useEffect(() => {
    const watched = new WeakSet<Element>();
    const watch = (el: HTMLElement) => {
      if (watched.has(el)) return;
      watched.add(el);
      fit(el);
      new ResizeObserver(() => fit(el)).observe(el);
    };
    const scan = (node: Node) => {
      if (!(node instanceof HTMLElement)) return;
      if (node.classList.contains("popover")) watch(node);
      node.querySelectorAll<HTMLElement>(".popover").forEach(watch);
    };
    const mo = new MutationObserver((records) => records.forEach((rec) => rec.addedNodes.forEach(scan)));
    mo.observe(document.body, { childList: true, subtree: true });
    document.querySelectorAll<HTMLElement>(".popover").forEach(watch);
    const onResize = () => document.querySelectorAll<HTMLElement>(".popover").forEach(fit);
    window.addEventListener("resize", onResize);
    return () => {
      mo.disconnect();
      window.removeEventListener("resize", onResize);
    };
  }, []);
}
