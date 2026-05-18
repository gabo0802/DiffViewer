import React, { useEffect, useRef, useState } from "react";

export function useResizablePane(initialWidth: number, minWidth: number, maxWidth: number) {
  const [width, setWidth] = useState(initialWidth);
  const [isResizing, setIsResizing] = useState(false);
  const startXRef = useRef(0);
  const startWidthRef = useRef(initialWidth);

  useEffect(() => {
    if (!isResizing) return;

    const onMouseMove = (event: MouseEvent) => {
      const delta = event.clientX - startXRef.current;
      setWidth(clamp(startWidthRef.current + delta, minWidth, maxWidth));
    };
    const onMouseUp = () => setIsResizing(false);

    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
    return () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };
  }, [isResizing, maxWidth, minWidth]);

  const beginResize = (event: React.MouseEvent<HTMLElement>) => {
    startXRef.current = event.clientX;
    startWidthRef.current = width;
    setIsResizing(true);
  };

  return { width, setWidth, isResizing, beginResize };
}

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}
