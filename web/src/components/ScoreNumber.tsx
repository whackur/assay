"use client";

import { useEffect, useState } from "react";

// The verdict reveal: the score counts up once on mount, ~700ms with an
// exponential ease-out, like a needle settling. Reduced-motion users get the
// final value immediately. Accessible labels on the parent always carry the
// final value, so assistive tech never hears the intermediate frames.

const DURATION_MS = 700;

function easeOut(t: number): number {
  return 1 - Math.pow(1 - t, 3);
}

export function ScoreNumber({ value }: { value: number }) {
  const [display, setDisplay] = useState(0);

  useEffect(() => {
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    let frame: number;
    const start = performance.now();
    const tick = (now: number) => {
      if (reduced) {
        setDisplay(value);
        return;
      }
      const t = Math.min(1, (now - start) / DURATION_MS);
      setDisplay(Math.round(easeOut(t) * value));
      if (t < 1) frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [value]);

  return <>{display}</>;
}
