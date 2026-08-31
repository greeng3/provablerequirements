// Phase 4d end-to-end smoke: drive the review workflow from the
// queue page all the way through approve → reject → resolve →
// re-request → approve. Runs against the same selenium-chrome
// container the Phase 1c smoke suite uses; skipped when selenium
// isn't reachable so CI hosts without it don't fail.

import { after, before, describe, it } from "node:test";
import assert from "node:assert/strict";

import { By, Key, until, type WebDriver } from "selenium-webdriver";

import { buildDriver, seleniumAvailable } from "./driver.ts";
import { readEnv } from "./env.ts";

describe("ReqForge review-workflow smoke", async () => {
  const env = readEnv();
  const available = await seleniumAvailable();

  if (!available) {
    it("skipped: selenium not reachable", { skip: true }, () => {
      console.log(
        `selenium at ${env.seleniumUrl} is not reachable — start a ` +
          `selenium-chrome container (docker run -d -p 4444:4444 ` +
          `selenium/standalone-chrome) before running the suite.`,
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

  it("drives the five review actions against the sample fixture", async () => {
    // Navigate to the review queue via the sidebar badge.
    await driver.get(`${env.reqforgeUrl}/reviews`);
    await driver.wait(
      until.elementLocated(By.css("h1#review-queue-heading")),
      env.stepTimeoutMs,
    );

    // The never-reviewed artifact (REQ-helloWorld) should appear
    // in the Awaiting-review section. Click through to it.
    const awaiting = await driver.findElement(
      By.css("a[href='/artifacts/0194f6d0-0001-7000-8000-000000000001']"),
    );
    await awaiting.click();
    await driver.wait(
      until.elementLocated(
        By.css("section[aria-labelledby='review-pane-heading']"),
      ),
      env.stepTimeoutMs,
    );

    // Approve.
    await clickByText(driver, "Approve");
    await fillReviewer(driver, "E2E reviewer");
    await submitDialog(driver, "Approve");
    await waitForStateBadge(driver, "approved");

    // Back to the queue — this artifact should be gone from
    // Awaiting-review now.
    await driver.get(`${env.reqforgeUrl}/reviews`);
    await driver.wait(
      until.elementLocated(By.css("h1#review-queue-heading")),
      env.stepTimeoutMs,
    );
    const stillAwaiting = await driver.findElements(
      By.css("a[href='/artifacts/0194f6d0-0001-7000-8000-000000000001']"),
    );
    assert.equal(
      stillAwaiting.length,
      0,
      "approved artifact should no longer appear in awaiting-review",
    );

    // Reject the greeting artifact with a TODO.
    await driver.get(
      `${env.reqforgeUrl}/artifacts/0194f6d0-0001-7000-8000-000000000002`,
    );
    await driver.wait(
      until.elementLocated(
        By.css("section[aria-labelledby='review-pane-heading']"),
      ),
      env.stepTimeoutMs,
    );
    await clickByText(driver, "Reject with TODO");
    await fillReviewer(driver, "E2E reviewer");
    await typeInto(driver, "Blocking TODO", "Add acceptance criteria");
    await submitDialog(driver, "Reject");
    await waitForStateBadge(driver, "rejected");

    // Resolve the blocking TODO from the inline popover.
    await clickByText(driver, "Resolve");
    // The popover reuses ReviewerSelect; pick the git default.
    await submitInlineResolve(driver);
    await waitForStateBadge(driver, "rejected"); // state unchanged until re-approval

    // Re-request review to push it back into the queue.
    await clickByText(driver, "Re-request review");
    await fillReviewer(driver, "E2E reviewer");
    await submitDialog(driver, "Re-request");
    await waitForStateBadge(driver, "re-review requested");

    // Finally approve — TODO is resolved, state flips to approved.
    await clickByText(driver, "Approve");
    await fillReviewer(driver, "E2E reviewer");
    await submitDialog(driver, "Approve");
    await waitForStateBadge(driver, "approved");
  });
});

async function clickByText(driver: WebDriver, text: string): Promise<void> {
  const button = await driver.wait(
    until.elementLocated(
      By.xpath(`//button[not(@disabled) and normalize-space()='${text}']`),
    ),
    5_000,
  );
  await button.click();
}

async function fillReviewer(driver: WebDriver, value: string): Promise<void> {
  // The ReviewerSelect starts as a <select>; "Type a new reviewer…"
  // flips it to a free-text <input> labelled "Reviewer identity".
  const select = await driver.wait(
    until.elementLocated(By.css("select[aria-label='Reviewer identity']")),
    5_000,
  );
  await select.sendKeys(Key.DOWN, Key.END, Key.ENTER);
  const input = await driver.wait(
    until.elementLocated(By.css("input[aria-label='Reviewer identity']")),
    5_000,
  );
  await input.clear();
  await input.sendKeys(value);
}

async function typeInto(
  driver: WebDriver,
  labelText: string,
  value: string,
): Promise<void> {
  const label = await driver.findElement(
    By.xpath(`//label[contains(normalize-space(), '${labelText}')]`),
  );
  const input = await label.findElement(By.css("input, textarea"));
  await input.clear();
  await input.sendKeys(value);
}

async function submitDialog(driver: WebDriver, label: string): Promise<void> {
  const submit = await driver.findElement(
    By.xpath(`//button[@type='submit' and normalize-space()='${label}']`),
  );
  await submit.click();
}

async function submitInlineResolve(driver: WebDriver): Promise<void> {
  const submit = await driver.findElement(
    By.xpath(`//button[@type='submit' and normalize-space()='Resolve']`),
  );
  await submit.click();
}

async function waitForStateBadge(
  driver: WebDriver,
  label: string,
): Promise<void> {
  await driver.wait(
    until.elementLocated(By.css(`[aria-label="review state: ${label}"]`)),
    10_000,
  );
}
