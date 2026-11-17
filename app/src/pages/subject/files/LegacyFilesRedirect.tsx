import { Navigate, useParams } from "react-router-dom";
import { filesFolderHref } from "@/lib/drive/routes";

/** `/subjects/<id>/downloads` and `/uploads`, which saved tabs may carry,
 *  sent to the Files tab's path of that name, which the tab redirects on
 *  (`resolveLegacyTab`). */
export function LegacyFilesRedirect({ tab }: { tab: string }) {
  const { subjectId } = useParams();
  return <Navigate to={filesFolderHref(Number(subjectId), tab)} replace />;
}
