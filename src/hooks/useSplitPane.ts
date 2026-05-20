import React, { useEffect, useRef, useState } from "react";

const FALLBACK_MIN_RATIO = 0.2;
const FALLBACK_MAX_RATIO = 0.8;

export function useSplitPane(initialRatio: number, minPaneWidth: number) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const [ratio, setRatio] = useState(() => clamp(initialRatio, FALLBACK_MIN_RATIO, FALLBACK_MAX_RATIO));
  const [isResizing, setIsResizing] = useState(false);

  useEffect(() => {
    if (!isResizing) return;

    const onMouseMove = (event: MouseEvent) => {
      const nextRatio = ratioFromClientX(containerRef.current, event.clientX);
      setRatio(clampRatio(nextRatio, containerRef.current, minPaneWidth));
    };
    const onMouseUp = () => setIsResizing(false);

    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
    return () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };
  }, [isResizing, minPaneWidth]);

  useEffect(() => {
    const onWindowResize = () => {
      setRatio((current) => clampRatio(current, containerRef.current, minPaneWidth));
    };

    window.addEventListener("resize", onWindowResize);
    return () => {
      window.removeEventListener("resize", onWindowResize);
    };
  }, [minPaneWidth]);

  const beginResize = (event: React.MouseEvent<HTMLElement>) => {
    event.preventDefault();
    setIsResizing(true);
    setRatio(clampRatio(ratioFromClientX(containerRef.current, event.clientX), containerRef.current, minPaneWidth));
  };

  return {
    containerRef,
    ratio,
    setRatio,
    isResizing,
    beginResize,
  };
}

function ratioFromClientX(container: HTMLDivElement | null, clientX: number) {
  if (!container) {
    return 0.5;
  }

  const rect = container.getBoundingClientRect();
  if (rect.width <= 0) {
    return 0.5;
  }

  return (clientX - rect.left) / rect.width;
}

function clampRatio(value: number, container: HTMLDivElement | null, minPaneWidth: number) {
  if (!container) {
    return clamp(value, FALLBACK_MIN_RATIO, FALLBACK_MAX_RATIO);
  }

  const width = container.getBoundingClientRect().width;
  if (!Number.isFinite(width) || width <= 0) {
    return clamp(value, FALLBACK_MIN_RATIO, FALLBACK_MAX_RATIO);
  }

  const minRatio = Math.min(0.5, minPaneWidth / width);
  const maxRatio = Math.max(0.5, 1 - minPaneWidth / width);
  return clamp(value, minRatio, maxRatio);
}

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}
