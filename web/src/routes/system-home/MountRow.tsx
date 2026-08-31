import { useState } from "react";
import { Link } from "react-router-dom";

import type { MountEntry } from "../../api/types";
import { InitProjectDialog } from "./InitProjectDialog";
import { MountBadge } from "./MountBadge";

const STATE_SUBTEXT: Record<Exclude<MountEntry["state"], "project">, string> = {
  needsInit:
    "This repository has no reqforge.json. Initialise it to start tracking artifacts.",
  noGit:
    "This directory is not a git repository. Run `git init` inside it, then reload.",
  loadFailed: "See the error message below for details.",
};

export function MountRow({ mount }: { mount: MountEntry }) {
  const [showInit, setShowInit] = useState(false);

  const body = (
    <div className="flex flex-1 items-start justify-between gap-4">
      <div className="min-w-0">
        <div className="flex items-center gap-2">
          <h3 className="truncate text-base font-medium text-slate-900 dark:text-slate-100">
            {mount.dirName}
          </h3>
          <MountBadge state={mount.state} />
        </div>
        <p className="mt-0.5 truncate font-mono text-xs text-slate-500">
          {mount.path}
        </p>
        {mount.state === "project" && mount.project ? (
          <p className="mt-1 text-sm text-slate-600 dark:text-slate-400">
            {mount.project.description
              ? `${mount.project.name} — ${mount.project.description}`
              : mount.project.name}
          </p>
        ) : mount.state === "loadFailed" ? (
          <p className="mt-1 text-sm text-rose-700 dark:text-rose-400">
            {mount.error ?? STATE_SUBTEXT.loadFailed}
          </p>
        ) : mount.state === "needsInit" || mount.state === "noGit" ? (
          <p className="mt-1 text-sm text-slate-600 dark:text-slate-400">
            {STATE_SUBTEXT[mount.state]}
          </p>
        ) : null}
      </div>
      <div className="flex shrink-0 flex-col items-end gap-1 text-right text-sm text-slate-500">
        {mount.state === "project" && mount.project ? (
          <>
            <div>
              <span className="font-semibold text-slate-900 dark:text-slate-100">
                {mount.project.collectionCount}
              </span>{" "}
              {mount.project.collectionCount === 1
                ? "collection"
                : "collections"}
            </div>
            <div>
              <span className="font-semibold text-slate-900 dark:text-slate-100">
                {mount.project.artifactCount}
              </span>{" "}
              {mount.project.artifactCount === 1 ? "artifact" : "artifacts"}
            </div>
          </>
        ) : mount.state === "needsInit" ? (
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              e.preventDefault();
              setShowInit(true);
            }}
            className="rounded bg-slate-900 px-2 py-1 text-xs text-white hover:bg-slate-700 dark:bg-slate-100 dark:text-slate-900"
          >
            Initialise
          </button>
        ) : null}
      </div>
    </div>
  );

  const row =
    mount.state === "project" && mount.project ? (
      <Link
        to={`/projects/${mount.project.slug}`}
        className="flex items-stretch rounded-lg border border-slate-200 bg-white p-4 shadow-sm transition hover:border-slate-300 hover:bg-slate-50 dark:border-slate-800 dark:bg-slate-900 dark:hover:border-slate-700 dark:hover:bg-slate-800"
      >
        {body}
      </Link>
    ) : (
      <div className="flex items-stretch rounded-lg border border-slate-200 bg-white p-4 shadow-sm dark:border-slate-800 dark:bg-slate-900">
        {body}
      </div>
    );

  return (
    <>
      {row}
      {showInit ? (
        <InitProjectDialog
          dirName={mount.dirName}
          onClose={() => setShowInit(false)}
        />
      ) : null}
    </>
  );
}
