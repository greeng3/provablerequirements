// Phase 5d artifact-shapes smoke: navigate to the DES (blob) and
// REF (url) collections shipped in the sample-project fixture, use
// the blob and URL views, and exercise the Check URL now action.
//
// Selenium-gated like the Phase 4d review-workflow suite — the
// test skips cleanly when no selenium is reachable so the
// default `npm test` run stays a no-op on machines without the
// container.

import { after, before, describe, it } from "node:test";
import assert from "node:assert/strict";

import { By, until, type WebDriver } from "selenium-webdriver";

import { buildDriver, seleniumAvailable } from "./driver.ts";
import { readEnv } from "./env.ts";

describe("Artifact shapes smoke suite", async () => {
  const env = readEnv();
  const available = await seleniumAvailable();

  if (!available) {
    it("skipped: selenium not reachable", { skip: true }, () => {
      console.log(
        `selenium at ${env.seleniumUrl} is not reachable — start a ` +
          `selenium-chrome container before running this suite.`,
      );
    });
    return;
  }

  let driver: WebDriver;

  before(async () => {
    driver = await buildDriver();
    driver.manage().setTimeouts({ implicit: env.stepTimeoutMs });
  });

  after(async () => {
    if (driver) await driver.quit();
  });

  it("renders the blob artifact with Download + Replace file actions", async () => {
    await driver.get(
      `${env.reqforgeUrl}/projects/sample-project/collections/DES/artifacts/DES-logo`,
    );
    await driver.wait(
      until.elementLocated(By.css("h1#artifact-heading")),
      env.stepTimeoutMs,
    );
    const title = await driver
      .findElement(By.css("h1#artifact-heading"))
      .getText();
    assert.equal(title, "ReqForge logo");

    // SVG is in the inline-preview tier — an <img> whose src
    // points at /api/artifacts/<uuid>/blob must render.
    const previewImg = await driver.wait(
      until.elementLocated(By.css("img[src*='/api/artifacts/'][src*='/blob']")),
      env.stepTimeoutMs,
    );
    const src = await previewImg.getAttribute("src");
    assert.ok(src?.includes("/blob"), `expected /blob URL, got ${src}`);

    // The Download link and Replace-file button sit in the
    // BlobHeader — match the visible labels rather than CSS
    // selectors so the test survives style refactors.
    const downloadLink = await driver.findElement(
      By.xpath("//a[normalize-space(text())='Download']"),
    );
    assert.ok(await downloadLink.isDisplayed());
    const replaceBtn = await driver.findElement(
      By.xpath("//button[normalize-space(text())='Replace file']"),
    );
    assert.ok(await replaceBtn.isDisplayed());
  });

  it("renders the URL artifact with a clickable link and Check URL now", async () => {
    await driver.get(
      `${env.reqforgeUrl}/projects/sample-project/collections/REF/artifacts/REF-rfc9110`,
    );
    await driver.wait(
      until.elementLocated(By.css("h1#artifact-heading")),
      env.stepTimeoutMs,
    );

    const urlLink = await driver.findElement(
      By.css("a[href='https://www.rfc-editor.org/rfc/rfc9110']"),
    );
    const target = await urlLink.getAttribute("target");
    assert.equal(target, "_blank");

    const checkBtn = await driver.findElement(
      By.xpath("//button[contains(., 'Check URL now')]"),
    );
    assert.ok(await checkBtn.isEnabled());
  });

  it("routes to the standalone diff view when navigating to /artifacts/:uuid/diff", async () => {
    // We don't know the UUID at test authorship time beyond the
    // fixture, but we can reach the diff route via the sidebar /
    // URL bar. Use the REF fixture UUID from the sidecar —
    // deliberately stable across the fixture's lifetime.
    const uuid = "0194f6d0-0003-7000-8000-000000000002";
    await driver.get(`${env.reqforgeUrl}/artifacts/${uuid}/diff`);
    await driver.wait(
      until.elementLocated(By.css("h1#diff-heading")),
      env.stepTimeoutMs,
    );
    const heading = await driver
      .findElement(By.css("h1#diff-heading"))
      .getText();
    // Title might be the cached title or the UUID if the detail
    // hasn't resolved yet; either value is a valid sanity check.
    assert.ok(heading.startsWith("Diff ·"));
  });
});
