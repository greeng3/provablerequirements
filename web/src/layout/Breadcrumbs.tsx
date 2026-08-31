import { Link, useLocation, useParams } from "react-router-dom";
import { Fragment } from "react";

interface Crumb {
  label: string;
  to?: string;
}

export function Breadcrumbs() {
  const location = useLocation();
  const params = useParams<{
    slug?: string;
    prefix?: string;
    name?: string;
    uuid?: string;
  }>();

  const crumbs: Crumb[] = [];
  const atRoot = location.pathname === "/";
  crumbs.push({
    label: "System",
    to: atRoot ? undefined : "/",
  });

  if (params.uuid && location.pathname.startsWith("/artifacts/")) {
    crumbs.push({ label: `Artifact ${shorten(params.uuid)}` });
    return renderCrumbs(crumbs);
  }

  if (params.slug) {
    const projectCrumb: Crumb = {
      label: params.slug,
      to: params.prefix ? `/projects/${params.slug}` : undefined,
    };
    crumbs.push(projectCrumb);

    if (params.prefix) {
      const collectionCrumb: Crumb = {
        label: params.prefix,
        to: params.name
          ? `/projects/${params.slug}/collections/${params.prefix}`
          : undefined,
      };
      crumbs.push(collectionCrumb);

      if (params.name) {
        crumbs.push({ label: params.name });
      }
    }
  }

  return renderCrumbs(crumbs);
}

function renderCrumbs(crumbs: Crumb[]) {
  return (
    <nav
      aria-label="Breadcrumb"
      className="border-b border-slate-200 bg-white px-6 py-2 text-sm dark:border-slate-800 dark:bg-slate-900"
    >
      <ol className="flex flex-wrap items-center gap-1 text-slate-600 dark:text-slate-400">
        {crumbs.map((crumb, i) => (
          <Fragment key={i}>
            {i > 0 ? (
              <li aria-hidden className="text-slate-400">
                /
              </li>
            ) : null}
            <li>
              {crumb.to ? (
                <Link
                  to={crumb.to}
                  className="hover:text-slate-900 hover:underline dark:hover:text-slate-100"
                >
                  {crumb.label}
                </Link>
              ) : (
                <span className="font-medium text-slate-900 dark:text-slate-100">
                  {crumb.label}
                </span>
              )}
            </li>
          </Fragment>
        ))}
      </ol>
    </nav>
  );
}

function shorten(uuid: string): string {
  return uuid.length > 8 ? `${uuid.slice(0, 8)}…` : uuid;
}
