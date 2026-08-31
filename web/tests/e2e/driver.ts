// WebDriver factory. Connects to the external selenium-chrome
// container configured via SELENIUM_URL. The driver is built fresh
// per test; the caller owns its lifecycle and quits it in afterEach.

import { Builder, Capabilities, type WebDriver } from "selenium-webdriver";

import { readEnv } from "./env.ts";

/// Build a remote WebDriver pointed at the configured selenium
/// container. Uses headless Chrome — the container renders the
/// pages, we just drive them.
export async function buildDriver(): Promise<WebDriver> {
  const { seleniumUrl } = readEnv();
  const capabilities = new Capabilities({
    browserName: "chrome",
    "goog:chromeOptions": {
      args: [
        "--headless=new",
        "--no-sandbox",
        "--disable-dev-shm-usage",
        "--window-size=1280,800",
      ],
    },
  });
  return await new Builder()
    .usingServer(seleniumUrl)
    .withCapabilities(capabilities)
    .build();
}

/// Quick reachability probe. Returns true when the selenium
/// container answers its /status endpoint within a small window.
/// Used by smoke.test.ts to skip cleanly when selenium isn't
/// running.
export async function seleniumAvailable(): Promise<boolean> {
  const { seleniumUrl } = readEnv();
  const statusUrl = seleniumUrl.replace(/\/wd\/hub\/?$/, "") + "/status";
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 1_000);
  try {
    const response = await fetch(statusUrl, { signal: controller.signal });
    return response.ok;
  } catch {
    return false;
  } finally {
    clearTimeout(timer);
  }
}
