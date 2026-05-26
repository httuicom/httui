import { useRef, useState, useCallback, useEffect } from "react";

const BOTTOM_THRESHOLD = 50;

export function useStickyScroll(deps: unknown[]) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const isAtBottom = useRef(true);
  const [showJumpButton, setShowJumpButton] = useState(false);

  const checkIfAtBottom = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return;
    const distanceFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
    const atBottom = distanceFromBottom < BOTTOM_THRESHOLD;
    isAtBottom.current = atBottom;
    setShowJumpButton(!atBottom);
  }, []);

  const scrollToBottom = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
    isAtBottom.current = true;
    setShowJumpButton(false);
  }, []);

  // Auto-scroll when deps change and user is at bottom
  useEffect(() => {
    if (isAtBottom.current) {
      scrollToBottom();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);

  // Attach scroll listener
  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    el.addEventListener("scroll", checkIfAtBottom);
    return () => el.removeEventListener("scroll", checkIfAtBottom);
  }, [checkIfAtBottom]);

  return { scrollRef, showJumpButton, scrollToBottom };
}
