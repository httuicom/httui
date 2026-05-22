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
      // Legacy TipTap sessions cached HTML; force a fresh read so CM6 doesn't render an HTML string.
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
            // `markUnsaved` mutates a Set in place (perf), so React doesn't see the dirty→clean
            // transition. Consumers subscribe to `saveSignal` to react to a save landing.
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
    // Cancel the pending auto-save so the same content isn't written twice.
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

  // Expose `forceSave` to non-React callers (e.g. DocHeader) without prop-drilling.
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
