import { Navigate } from "react-router-dom";

import { useProjects } from "../api/queries";

/// Single-subject entry point. The backend serves exactly one
/// project, so `/` resolves it and sends the operator straight to
/// its page — there is no multi-project landing surface.
export function HomeRedirect() {
  const projectsQuery = useProjects();

  if (projectsQuery.isLoading) {
    return (
      <p className="text-sm text-slate-500" role="status">
        Loading…
      </p>
    );
  }

  const project = projectsQuery.data?.[0];
  if (projectsQuery.isError || !project) {
    return (
      <p
        className="text-sm text-slate-600 dark:text-slate-400"
        role="alert"
      >
        No project found.
      </p>
    );
  }

  return <Navigate replace to={`/projects/${project.slug}`} />;
}
