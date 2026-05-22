import { useCallback, useEffect, useRef, useState } from "react";

import { listTemplates, type Template } from "@/lib/tauri/templates";

export interface UseTemplatesResult {
  templates: Template[];
  loaded: boolean;
  error: string | null;
  refresh: () => void;
}

export function useTemplates(vaultPath: string | null): UseTemplatesResult {
  const [templates, setTemplates] = useState<Template[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const cancelledRef = useRef(false);

  const fetchOnce = useCallback(async () => {
    if (!vaultPath) {
      setTemplates([]);
      setLoaded(false);
      setError(null);
      return;
    }
    try {
      const next = await listTemplates(vaultPath);
      if (cancelledRef.current) return;
      setTemplates(next);
      setLoaded(true);
      setError(null);
    } catch (e) {
      if (cancelledRef.current) return;
      setTemplates([]);
      setLoaded(false);
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [vaultPath]);

  useEffect(() => {
    cancelledRef.current = false;
    void fetchOnce();
    return () => {
      cancelledRef.current = true;
    };
  }, [fetchOnce]);

  const refresh = useCallback(() => {
    void fetchOnce();
  }, [fetchOnce]);

  return { templates, loaded, error, refresh };
}
