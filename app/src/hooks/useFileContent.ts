import { useState, useCallback } from "react";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import type { DbFile } from "@/lib/db";

export function useFileContent(dataDir: string) {
  const [activeFile, setActiveFile] = useState<DbFile | null>(null);
  const [content, setContent] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const assetUrl = useCallback(
    (relativePath: string) => {
      if (!dataDir) return "";
      const abs = `${dataDir}/${relativePath}`.replace(/\/{2,}/g, "/");
      return convertFileSrc(abs);
    },
    [dataDir],
  );

  const openFile = useCallback(async (file: DbFile) => {
    setActiveFile(file);
    setError(null);
    if (file.category === "image") {
      setContent("");
      setLoading(false);
      return;
    }
    if (file.category === "file") {
      setContent("");
      setLoading(false);
      invoke("open_course_file", { relativePath: file.relative_path }).catch(
        (err) => setError(String(err)),
      );
      return;
    }
    setLoading(true);
    try {
      const text = await invoke<string>("read_course_file", {
        relativePath: file.relative_path,
      });
      setContent(text);
    } catch (err) {
      setError(String(err));
      setContent("");
    } finally {
      setLoading(false);
    }
  }, []);

  return { activeFile, content, loading, error, assetUrl, openFile, setActiveFile };
}
