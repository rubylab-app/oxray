import {
  elementScroll,
  observeElementOffset,
  observeElementRect,
  Virtualizer,
} from "@tanstack/virtual-core";
import type { RefObject } from "preact";
import { useLayoutEffect, useRef, useState } from "preact/hooks";

export function useRowVirtualizer(
  count: number,
  parentRef: RefObject<HTMLDivElement | null>,
): Virtualizer<HTMLDivElement, Element> {
  const [, setTick] = useState(0);
  const virtualizerRef = useRef<Virtualizer<HTMLDivElement, Element> | null>(
    null,
  );
  if (virtualizerRef.current === null) {
    virtualizerRef.current = new Virtualizer({
      count,
      getScrollElement: () => parentRef.current,
      estimateSize: () => 32,
      overscan: 12,
      scrollToFn: elementScroll,
      observeElementRect,
      observeElementOffset,
      onChange: () => {
        setTick((value) => value + 1);
      },
    });
  }
  const virtualizer = virtualizerRef.current;
  virtualizer.setOptions({
    count,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 32,
    overscan: 12,
    scrollToFn: elementScroll,
    observeElementRect,
    observeElementOffset,
    onChange: () => {
      setTick((value) => value + 1);
    },
  });
  useLayoutEffect(() => virtualizer._didMount(), [virtualizer]);
  useLayoutEffect(() => {
    virtualizer._willUpdate();
  });
  return virtualizer;
}
