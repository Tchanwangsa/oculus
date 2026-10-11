import { invoke } from "@tauri-apps/api/core";

export const basename = (path: string) => path.slice(path.lastIndexOf("/") + 1);

/** The markdown this embed stands for; a path with a space needs `<…>`. */
export function sourceOf(path: string, alt?: string): string {
  const target = /[\s()<>]/.test(path) ? `<${path}>` : path;
  return `![${(alt ?? "").replace(/[[\]]/g, "\\$&")}](${target})`;
}

export function openExternally(path: string) {
  invoke("open_course_file", { relativePath: path }).catch(console.error);
}
