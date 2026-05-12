import { useCallback, useEffect, useRef } from "react";
import { readNote, writeNote, setConfig } from "@/lib/tauri/commands";
import { setActiveFileSaver } from "@/lib/active-file-save";
import { usePaneStore } from "@/stores/pane";
import { useTagIndexStore } from "@/stores/tagIndex";
import { useWorkspaceStore } from "@/stores/workspace";
import { useSettingsStore } from "@/stores/settings";

export function useEditorSession() {
  const autoSaveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const suppressedFiles = useRef<Set<string>>(new Set());

  const handleFileSelect = useCallback(async (filePath: string) => {
    const vaultPath = useWorkspaceStore.getState().vaultPath;
    if (!vaultPath) return;
    try {
      const { editorContents, openFile } = usePaneStore.getState();
      const cached = editorContents.get(filePath);
      // Legacy TipTap sessions cached HTML — detect and force a fresh
      // markdown read so the CM6 editor doesn't render an HTML string.
      const needsRead = !cached || cached.trimStart().startsWith("<");
      if (needsRead) {
        const markdown = await readNote(vaultPath, filePath);
        openFile(filePath, markdown, vaultPath);
      } else {
        openFile(filePath, cached, vaultPath);
      }
      setConfig("active_file", filePath).catch(() => {});
    } catch (err) {
      console.error("Failed to read note:", err);
    }
  }, []);

  const handleEditorChange = useCallback(
    (
      _paneId: string,
      filePath: string,
      content: string,
      tabVaultPath: string,
    ) => {
      const { updateContent, markUnsaved, activePaneId } =
        usePaneStore.getState();
      const {
        settings: { autoSaveMs },
      } = useSettingsStore.getState();
      updateContent(filePath, content);
      markUnsaved(activePaneId, filePath, true);

      if (autoSaveTimer.current) clearTimeout(autoSaveTimer.current);

      if (autoSaveMs > 0) {
        autoSaveTimer.current = setTimeout(async () => {
          if (usePaneStore.getState().hasConflict(filePath)) return;
          if (suppressedFiles.current.has(filePath)) return;
          try {
            await writeNote(tabVaultPath, filePath, content);
            const store = usePaneStore.getState();
            store.markUnsaved(store.activePaneId, filePath, false);
            // Reactive notification — `markUnsaved` mutates a Set in
            // place (perf), so React doesn't see the dirty→clean
            // transition. Consumers that need to react to "a save just
            // landed" (e.g. `useFilePreflight` re-fetch) subscribe to
            // `saveSignal` instead.
            store.notifySaved();
            useTagIndexStore.getState().refreshTagsForFile(filePath, content);
          } catch (err) {
            console.error("Auto-save failed:", err);
          }
        }, autoSaveMs);
      }
    },
    [],
  );

  const forceSave = useCallback(() => {
    // Cancel any pending auto-save timer so we don't end up writing
    // the same content twice (once now, once 1s later when the timer
    // fires). The discrete callers — keyboard Cmd+S, DocHeader
    // actions — own the save moment.
    if (autoSaveTimer.current) {
      clearTimeout(autoSaveTimer.current);
      autoSaveTimer.current = null;
    }
    const { getActiveLeaf, editorContents, markUnsaved } =
      usePaneStore.getState();
    const leaf = getActiveLeaf();
    if (!leaf || leaf.tabs.length === 0) return;
    const tab = leaf.tabs[leaf.activeTab];
    if (!tab) return;
    const content = editorContents.get(tab.filePath);
    if (content) {
      const filePath = tab.filePath;
      writeNote(tab.vaultPath, filePath, content)
        .then(() => {
          markUnsaved(leaf.id, filePath, false);
          usePaneStore.getState().notifySaved();
          useTagIndexStore.getState().refreshTagsForFile(filePath, content);
        })
        .catch((err) => console.error("Save failed:", err));
    }
  }, []);

  // Register `forceSave` as the active-file saver so non-React code
  // paths (DocHeader callbacks, etc.) can trigger an immediate write
  // without prop-drilling through the component tree.
  useEffect(() => {
    setActiveFileSaver(forceSave);
    return () => setActiveFileSaver(null);
  }, [forceSave]);

  const suppressAutoSave = useCallback((filePath: string) => {
    if (autoSaveTimer.current) {
      clearTimeout(autoSaveTimer.current);
      autoSaveTimer.current = null;
    }
    suppressedFiles.current.add(filePath);
  }, []);

  const unsuppressAutoSave = useCallback((filePath: string) => {
    suppressedFiles.current.delete(filePath);
  }, []);

  return {
    handleFileSelect,
    handleEditorChange,
    forceSave,
    suppressAutoSave,
    unsuppressAutoSave,
  };
}
