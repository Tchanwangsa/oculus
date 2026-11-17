import type { DbFile } from "@/lib/db";
import { categoryOf, courseRelOf } from "@/lib/files/libraryLayout";

/** Category and filename derived from a library path — enough for
 *  `categoryIconFor` and `fileTitle` without a query. */
export function pathFile(path: string): Pick<DbFile, "category" | "filename"> {
  const rel = courseRelOf(path);
  return { category: categoryOf(rel), filename: rel.slice(rel.lastIndexOf("/") + 1) };
}
