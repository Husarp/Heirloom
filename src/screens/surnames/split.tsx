// Pieces shared by the list + detail screens Nazwiska and Miejsca (spec §2.9, §5.9, §5.10). Nothing here is
// specific to surnames or places, so it can move to src/components when another screen needs it.

import { useVirtualizer } from "@tanstack/react-virtual";
import { useEffect, useLayoutEffect, useState, type ReactNode, type RefObject } from "react";
import { useStore } from "../../app/store";
import "./split.css";

/** Lower case without Polish letters, for search: "Łęczna" → "leczna". */
export function fold(text: string): string {
  return text.toLowerCase().normalize("NFD").replace(/[̀-ͯ]/g, "").replace(/ł/g, "l");
}

/** Polish alphabetical order ("Łęczna" after "Lublin", case ignored). */
export const collator = new Intl.Collator("pl", { sensitivity: "base" });

/** Sets the last breadcrumb part. Screen clears the crumb after every route change, and its effect runs after
 *  ours, so this puts it back whenever it has been cleared. */
export function useCrumb(text: string | null) {
  const crumb = useStore((s) => s.crumb);
  const setCrumb = useStore((s) => s.setCrumb);
  useEffect(() => {
    if (text && crumb !== text) setCrumb(text);
  }, [text, crumb, setCrumb]);
}

/** Fixed-height virtual rows of a list inside a scrolling pane that also holds other content above the list. */
export function usePaneRows(count: number, rowHeight: number, pane: RefObject<HTMLElement | null>, list: RefObject<HTMLElement | null>) {
  const [margin, setMargin] = useState(0);
  useLayoutEffect(() => {
    // The list's distance from the top of the pane's content. offsetTop uses the same units as scrollTop even
    // when the text size setting zooms the page (getBoundingClientRect wouldn't). The pane is position: relative,
    // so the offsetParent chain ends there.
    let top = 0;
    let el: HTMLElement | null = list.current;
    while (el && el !== pane.current) {
      top += el.offsetTop;
      el = el.offsetParent as HTMLElement | null;
    }
    if (el && top !== margin) setMargin(top);
  });
  return useVirtualizer({ count, getScrollElement: () => pane.current, estimateSize: () => rowHeight, overscan: 8, scrollMargin: margin });
}

/** „Podobne: Kowalewski (2 osoby). Dołączyć do Kowalskich?” with its buttons. */
export function SuggestionBar({ icon, children, actions }: { icon: ReactNode; children: ReactNode; actions?: ReactNode }) {
  return (
    <div className="suggest">
      {icon}
      <span className="grow">{children}</span>
      {actions}
    </div>
  );
}
