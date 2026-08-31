// Phase 1c smoke suite: walk from System Home down to an artifact
// against a live ReqForge backend + frontend, driving an external
// selenium-chrome container.
//
// The four scenarios from the ROADMAP are implemented as a single
// click-through flow with assertions at each step; running them as
// one test keeps the browser-startup cost paid once.

import { after, before, describe, it } from "node:test";
import assert from "node:assert/strict";

import { By, until, type WebDriver } from "selenium-webdriver";

import { buildDriver, seleniumAvailable } from "./driver.ts";
import { readEnv } from "./env.ts";

describe("ReqForge smoke suite", async () => {
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

  it("walks from System Home down to the Hello World artifact", async () => {
    // 1. System Home renders the sample project with the
    //    correct validity-state badge.
    await driver.get(env.reqforgeUrl);

    await driver.wait(
      until.elementLocated(By.css("h1#system-home-heading")),
      env.stepTimeoutMs,
    );

    const sampleLink = await driver.wait(
      until.elementLocated(By.css("a[href='/projects/sample-project']")),
      env.stepTimeoutMs,
    );
    const projectBadge = await driver.findElement(
      By.css("[aria-label='mount state: Project']"),
    );
    assert.equal(await projectBadge.getText(), "Project");

    // 2. Click into the project. Collections render.
    await sampleLink.click();
    await driver.wait(
      until.elementLocated(By.css("h1#project-heading")),
      env.stepTimeoutMs,
    );
    const projectHeading = await driver
      .findElement(By.css("h1#project-heading"))
      .getText();
    assert.equal(projectHeading, "Sample Project");

    const collectionLink = await driver.findElement(
      By.css("a[href='/projects/sample-project/collections/REQ']"),
    );

    // 3. Click into the collection. The artifact list renders.
    await collectionLink.click();
    await driver.wait(
      until.elementLocated(By.css("h1#collection-heading")),
      env.stepTimeoutMs,
    );
    const collectionHeading = await driver
      .findElement(By.css("h1#collection-heading"))
      .getText();
    assert.match(collectionHeading, /REQ\s+Requirements/);

    const artifactLink = await driver.wait(
      until.elementLocated(
        By.css(
          "a[href='/projects/sample-project/collections/REQ/artifacts/REQ-helloWorld']",
        ),
      ),
      env.stepTimeoutMs,
    );

    // 4. Click into the artifact. Title and Markdown body appear
    //    as rendered HTML, not raw text.
    await artifactLink.click();
    await driver.wait(
      until.elementLocated(By.css("h1#artifact-heading")),
      env.stepTimeoutMs,
    );
    const artifactTitle = await driver
      .findElement(By.css("h1#artifact-heading"))
      .getText();
    assert.equal(artifactTitle, "Hello World requirement");

    // The body's `# Hello World` Markdown must render as an <h1>
    // inside the .prose article — not as literal "# Hello World"
    // text. We find any h1 in the article whose text matches.
    const bodyHeading = await driver.wait(
      until.elementLocated(By.css("article.prose h1")),
      env.stepTimeoutMs,
    );
    assert.equal(await bodyHeading.getText(), "Hello World");

    // Sanity: the raw "# " prefix must not appear in the article
    // text (that would mean react-markdown didn't process it).
    const articleText = await driver
      .findElement(By.css("article.prose"))
      .getText();
    assert.ok(
      !articleText.includes("# Hello World"),
      `Markdown did not render — article text was: ${articleText.slice(0, 200)}`,
    );
  });
});
