import { Navigate, useParams } from "react-router-dom";
import { parseSubjectParam, tasksHref } from "@/components/projects/tasks/taskViews";

/** Saved subject planning URLs open the central Tasks page in that subject. */
export default function SubjectTasksRedirect() {
  const { subjectId } = useParams();
  return <Navigate to={tasksHref("all", null, parseSubjectParam(subjectId ?? null))} replace />;
}
