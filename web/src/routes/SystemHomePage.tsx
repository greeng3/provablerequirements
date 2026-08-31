import { useMounts } from "../api/queries";
import { EmptyMountsState } from "./system-home/EmptyMountsState";
import { MountRow } from "./system-home/MountRow";
import { SystemConfigBanner } from "./system-home/SystemConfigBanner";

export function SystemHomePage() {
  const mounts = useMounts();

  return (
    <section aria-labelledby="system-home-heading">
      <h1
        id="system-home-heading"
        className="text-2xl font-semibold tracking-tight"
      >
        System Home
      </h1>
      <p className="mt-1 text-sm text-slate-600 dark:text-slate-400">
        Every directory discovered under the ReqForge mount prefix.
      </p>

      <div className="mt-6">
        <SystemConfigBanner />
      </div>

      <div className="space-y-3">
        {mounts.isLoading ? (
          <p className="text-sm text-slate-500">Discovering mounts…</p>
        ) : mounts.isError ? (
          <p className="text-sm text-rose-600" role="alert">
            Could not reach the ReqForge backend:{" "}
            {String(mounts.error ?? "unknown error")}
          </p>
        ) : mounts.data && mounts.data.length > 0 ? (
          mounts.data.map((mount) => (
            <MountRow key={mount.path} mount={mount} />
          ))
        ) : (
          <EmptyMountsState />
        )}
      </div>
    </section>
  );
}
