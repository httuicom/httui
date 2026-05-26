import { useState, useCallback } from "react";
import {
  createNote,
  createFolder,
  deleteNote,
  renameNote,
} from "@/lib/tauri/commands";
import { useTagIndexStore } from "@/stores/tagIndex";

export interface InlineCreate {
  type: "note" | "folder";
  dirPath: string;
}

interface UseFileOperationsOpts {
  vaultPath: string | null;
  refreshFileTree: (vault: string) => Promise<void>;
  onFileCreated?: (filePath: string) => void;
}

export function useFileOperations({
  vaultPath,
  refreshFileTree,
  onFileCreated,
}: UseFileOperationsOpts) {
  const [inlineCreate, setInlineCreate] = useState<InlineCreate | null>(null);

  const handleStartCreate = useCallback(
    (type: "note" | "folder", dirPath: string) => {
      setInlineCreate({ type, dirPath });
    },
    [],
  );

  const cancelInlineCreate = useCallback(() => {
    setInlineCreate(null);
  }, []);

  const handleCreateNote = useCallback(
    async (dirPath: string, name: string) => {
      if (!vaultPath || !name) return;
      setInlineCreate(null);
      const filePath = dirPath ? `${dirPath}/${name}.md` : `${name}.md`;
      try {
        await createNote(vaultPath, filePath);
        await refreshFileTree(vaultPath);
        onFileCreated?.(filePath);
      } catch (err) {
        console.error("Failed to create note:", err);
      }
    },
    [vaultPath, refreshFileTree, onFileCreated],
  );

  const handleCreateFolder = useCallback(
    async (dirPath: string, name: string) => {
      if (!vaultPath || !name) return;
      setInlineCreate(null);
      const folderPath = dirPath ? `${dirPath}/${name}` : name;
      try {
        await createFolder(vaultPath, folderPath);
        await refreshFileTree(vaultPath);
      } catch (err) {
        console.error("Failed to create folder:", err);
      }
    },
    [vaultPath, refreshFileTree],
  );

  const handleRename = useCallback(
    async (path: string, newName: string) => {
      if (!vaultPath || !newName) return;
      const dir = path.includes("/")
        ? path.substring(0, path.lastIndexOf("/"))
        : "";
      const newPath = dir ? `${dir}/${newName}` : newName;
      try {
        await renameNote(vaultPath, path, newPath);
        await refreshFileTree(vaultPath);
        // Tag index is keyed by old path — drop it. The next save
        // (or the vault-walker re-run) re-indexes under the new
        // path. Cheap; keeps quick-open #tag results coherent.
        useTagIndexStore.getState().removeFile(path);
      } catch (err) {
        console.error("Failed to rename:", err);
      }
    },
    [vaultPath, refreshFileTree],
  );

  const handleDelete = useCallback(
    async (path: string) => {
      if (!vaultPath) return;
      try {
        await deleteNote(vaultPath, path);
        await refreshFileTree(vaultPath);
        useTagIndexStore.getState().removeFile(path);
      } catch (err) {
        console.error("Failed to delete:", err);
      }
    },
    [vaultPath, refreshFileTree],
  );

  const handleMoveFile = useCallback(
    async (sourcePath: string, targetDir: string) => {
      if (!vaultPath) return;
      const fileName = sourcePath.includes("/")
        ? sourcePath.substring(sourcePath.lastIndexOf("/") + 1)
        : sourcePath;
      const newPath = targetDir ? `${targetDir}/${fileName}` : fileName;
      if (newPath === sourcePath) return;
      try {
        await renameNote(vaultPath, sourcePath, newPath);
        await refreshFileTree(vaultPath);
        // Drop the old path's tag entry — same semantic as
        // handleRename. Next save under the new path re-indexes.
        useTagIndexStore.getState().removeFile(sourcePath);
      } catch (err) {
        console.error("Failed to move file:", err);
      }
    },
    [vaultPath, refreshFileTree],
  );

  return {
    inlineCreate,
    handleStartCreate,
    handleCreateNote,
    handleCreateFolder,
    handleRename,
    handleDelete,
    handleMoveFile,
    cancelInlineCreate,
  };
}
