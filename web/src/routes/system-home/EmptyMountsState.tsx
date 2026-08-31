/// Empty-state guidance per UX-startupHomeView: when no mounts are
/// discovered, show concrete next-step instructions (a compose
/// snippet plus a docker run equivalent) rather than a bare
/// "no items" message.
export function EmptyMountsState() {
  return (
    <div className="rounded-lg border border-dashed border-slate-300 bg-white p-8 text-sm text-slate-700 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-300">
      <h2 className="text-base font-semibold text-slate-900 dark:text-slate-100">
        No repositories mounted
      </h2>
      <p className="mt-2">
        ReqForge didn't find any directories under its mount prefix. Add bind
        mounts for each repository you want to manage.
      </p>

      <p className="mt-4 font-semibold">docker-compose.yml</p>
      <pre className="mt-1 overflow-x-auto rounded bg-slate-100 p-3 font-mono text-xs dark:bg-slate-800">
        {`services:
  reqforge:
    image: reqforge:local
    volumes:
      - ./your-repo:/repos/your-repo`}
      </pre>

      <p className="mt-4 font-semibold">docker run</p>
      <pre className="mt-1 overflow-x-auto rounded bg-slate-100 p-3 font-mono text-xs dark:bg-slate-800">
        {`docker run --rm \\
  -p 36743:36743 \\
  -v $(pwd)/your-repo:/repos/your-repo \\
  reqforge:local`}
      </pre>

      <p className="mt-4">
        Reload this page once the container sees the mount.
      </p>
    </div>
  );
}
