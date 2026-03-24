import { useCallback, useEffect, useRef, useState } from 'react';

export function useResizable(initial: number, min: number, max: number, reverse = false) {
  const [size, setSize] = useState(initial);
  const dragging = useRef(false);
  const startX = useRef(0);
  const startSize = useRef(0);

  const onMouseDown = useCallback((e: React.MouseEvent) => {
    dragging.current = true;
    startX.current = e.clientX;
    startSize.current = size;
    e.preventDefault();
  }, [size]);

  useEffect(() => {
    const onMove = (e: MouseEvent) => {
      if (!dragging.current) return;
      const delta = e.clientX - startX.current;
      setSize(Math.min(max, Math.max(min, startSize.current + (reverse ? -delta : delta))));
    };
    const onUp = () => { dragging.current = false; };
    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
    return () => {
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', onUp);
    };
  }, [min, max, reverse]);

  return { size, onMouseDown };
}
