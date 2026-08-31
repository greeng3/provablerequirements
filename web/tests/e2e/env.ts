// Environment-driven configuration for the e2e smoke suite.
//
// Every value has a sensible default so `npm run test:e2e` works
// against a locally-running backend + selenium-chrome container.
// CI environments override via env vars.

export interface E2eConfig {
  /// URL where ReqForge is reachable (backend + frontend, same origin).
  readonly reqforgeUrl: string;
  /// WebDriver endpoint of the selenium-chrome container.
  readonly seleniumUrl: string;
  /// Wall-clock timeout for each smoke step, in milliseconds.
  readonly stepTimeoutMs: number;
}

export function readEnv(): E2eConfig {
  return {
    reqforgeUrl: process.env.REQFORGE_E2E_URL ?? "http://localhost:36743",
    seleniumUrl: process.env.SELENIUM_URL ?? "http://localhost:4444/wd/hub",
    stepTimeoutMs: Number(process.env.E2E_STEP_TIMEOUT_MS ?? 10_000),
  };
}
