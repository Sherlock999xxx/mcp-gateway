"use client";

import { useCallback, useEffect, useRef } from "react";

export function useQueuedAutosave<T>({
  isPending,
  mutate,
  computeKey,
}: {
  isPending: boolean;
  mutate: (value: T) => void;
  computeKey: (value: T) => string;
}) {
  const lastSavedKeyRef = useRef<string>("");
  const queuedRef = useRef<T | null>(null);

  const setLastSavedKey = useCallback((key: string) => {
    lastSavedKeyRef.current = key;
  }, []);

  const commit = useCallback(
    (next: T) => {
      const key = computeKey(next);
      if (key === lastSavedKeyRef.current) return;

      if (isPending) {
        queuedRef.current = next;
        return;
      }

      lastSavedKeyRef.current = key;
      mutate(next);
    },
    [computeKey, isPending, mutate],
  );

  useEffect(() => {
    if (isPending) return;
    const queued = queuedRef.current;
    if (!queued) return;
    queuedRef.current = null;
    commit(queued);
  }, [commit, isPending]);

  return { commit, setLastSavedKey };
}
