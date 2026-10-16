import { useEffect, useRef, useState } from "react";

export function useWidth<T extends HTMLElement>() {
  const held = useRef<T>(null);
  const [width, setWidth] = useState(0);

  useEffect(() => {
    const element = held.current;
    if (!element) return;
    setWidth(element.clientWidth);
    const watcher = new ResizeObserver(([entry]) => {
      if (entry) setWidth(entry.contentRect.width);
    });
    watcher.observe(element);
    return () => watcher.disconnect();
  }, []);

  return { held, width };
}
