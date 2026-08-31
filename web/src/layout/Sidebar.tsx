import { NavLink, useParams } from "react-router-dom";
import clsx from "clsx";

import { useCollections, useProjects, useReviewQueue } from "../api/queries";

/// Sidebar driven by the current URL params: when a project is
/// selected, its collections appear nested beneath it.
export function Sidebar() {
  const params = useParams<{ slug?: string }>();
  const projectsQuery = useProjects();
  const collectionsQuery = useCollections(params.slug);
  const queueQuery = useReviewQueue({});
  const queueCount =
    (queueQuery.data?.awaitingReview?.length ?? 0) +
    (queueQuery.data?.blockingTodos?.length ?? 0);

  return (
    <aside
      aria-label="Projects and collections"
      className="hidden w-64 shrink-0 border-r border-slate-200 bg-white p-4 md:block dark:border-slate-800 dark:bg-slate-900"
    >
      <NavLink
        to="/search"
        className={({ isActive }) =>
          clsx(
            "mb-2 block rounded px-2 py-1 text-sm",
            isActive
              ? "bg-slate-100 font-medium text-slate-900 dark:bg-slate-800 dark:text-slate-100"
              : "text-slate-700 hover:bg-slate-50 dark:text-slate-300 dark:hover:bg-slate-800",
          )
        }
      >
        Search
      </NavLink>
      <NavLink
        to="/browse"
        className={({ isActive }) =>
          clsx(
            "mb-2 block rounded px-2 py-1 text-sm",
            isActive
              ? "bg-slate-100 font-medium text-slate-900 dark:bg-slate-800 dark:text-slate-100"
              : "text-slate-700 hover:bg-slate-50 dark:text-slate-300 dark:hover:bg-slate-800",
          )
        }
      >
        Browse
      </NavLink>
      <NavLink
        to="/reviews"
        className={({ isActive }) =>
          clsx(
            "mb-2 flex items-center justify-between rounded px-2 py-1 text-sm",
            isActive
              ? "bg-slate-100 font-medium text-slate-900 dark:bg-slate-800 dark:text-slate-100"
              : "text-slate-700 hover:bg-slate-50 dark:text-slate-300 dark:hover:bg-slate-800",
          )
        }
      >
        <span>Review queue</span>
        {queueCount > 0 ? (
          <span
            className="ml-2 rounded-full bg-amber-100 px-2 text-xs text-amber-800 dark:bg-amber-900/50 dark:text-amber-100"
            aria-label={`${queueCount} items in review queue`}
          >
            {queueCount}
          </span>
        ) : null}
      </NavLink>
      <NavLink
        to="/reports"
        className={({ isActive }) =>
          clsx(
            "mb-2 block rounded px-2 py-1 text-sm",
            isActive
              ? "bg-slate-100 font-medium text-slate-900 dark:bg-slate-800 dark:text-slate-100"
              : "text-slate-700 hover:bg-slate-50 dark:text-slate-300 dark:hover:bg-slate-800",
          )
        }
      >
        Reports
      </NavLink>
      <NavLink
        to="/explore/graph"
        className={({ isActive }) =>
          clsx(
            "mb-2 block rounded px-2 py-1 text-sm",
            isActive
              ? "bg-slate-100 font-medium text-slate-900 dark:bg-slate-800 dark:text-slate-100"
              : "text-slate-700 hover:bg-slate-50 dark:text-slate-300 dark:hover:bg-slate-800",
          )
        }
      >
        Graph
      </NavLink>
      <NavLink
        to="/explore/matrix"
        className={({ isActive }) =>
          clsx(
            "mb-2 block rounded px-2 py-1 text-sm",
            isActive
              ? "bg-slate-100 font-medium text-slate-900 dark:bg-slate-800 dark:text-slate-100"
              : "text-slate-700 hover:bg-slate-50 dark:text-slate-300 dark:hover:bg-slate-800",
          )
        }
      >
        Matrix
      </NavLink>
      <NavLink
        to="/llm"
        data-testid="sidebar-llm-link"
        className={({ isActive }) =>
          clsx(
            "mb-4 block rounded px-2 py-1 text-sm",
            isActive
              ? "bg-slate-100 font-medium text-slate-900 dark:bg-slate-800 dark:text-slate-100"
              : "text-slate-700 hover:bg-slate-50 dark:text-slate-300 dark:hover:bg-slate-800",
          )
        }
      >
        LLM providers
      </NavLink>
      <h2 className="mb-2 text-xs font-semibold tracking-wide text-slate-500 uppercase">
        Projects
      </h2>
      {projectsQuery.isLoading ? (
        <p className="text-sm text-slate-500">Loading…</p>
      ) : projectsQuery.isError ? (
        <p className="text-sm text-rose-600">Failed to load projects.</p>
      ) : projectsQuery.data && projectsQuery.data.length > 0 ? (
        <ul className="space-y-1 text-sm">
          {projectsQuery.data.map((project) => {
            const isActive = params.slug === project.slug;
            return (
              <li key={project.slug}>
                <NavLink
                  to={`/projects/${project.slug}`}
                  className={({ isActive: active }) =>
                    clsx(
                      "block rounded px-2 py-1",
                      active
                        ? "bg-slate-100 font-medium text-slate-900 dark:bg-slate-800 dark:text-slate-100"
                        : "text-slate-700 hover:bg-slate-50 dark:text-slate-300 dark:hover:bg-slate-800",
                    )
                  }
                >
                  {project.name}
                </NavLink>
                {isActive && collectionsQuery.data ? (
                  <ul className="mt-1 ml-3 space-y-0.5 border-l border-slate-200 pl-3 text-sm dark:border-slate-700">
                    {collectionsQuery.data.map((collection) => (
                      <li key={collection.prefix}>
                        <NavLink
                          to={`/projects/${project.slug}/collections/${collection.prefix}`}
                          className={({ isActive }) =>
                            clsx(
                              "block rounded px-2 py-0.5 font-mono text-xs",
                              isActive
                                ? "bg-slate-100 text-slate-900 dark:bg-slate-800 dark:text-slate-100"
                                : "text-slate-600 hover:bg-slate-50 dark:text-slate-400 dark:hover:bg-slate-800",
                            )
                          }
                        >
                          {collection.prefix}
                        </NavLink>
                      </li>
                    ))}
                  </ul>
                ) : null}
              </li>
            );
          })}
        </ul>
      ) : (
        <p className="text-sm text-slate-500">No projects mounted.</p>
      )}
    </aside>
  );
}
