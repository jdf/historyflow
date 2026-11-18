// e2e driver: one pinned Chrome for Testing instance serves every case.
// For each name|query pair, navigate, wait for the page's ready flag,
// and capture a PNG. Any console error or uncaught exception fails the
// run. Driven by tools/e2e.sh.
//
//   node tools/e2e.mjs <chrome> <base-url> <out-dir> name|query...
import puppeteer from "puppeteer-core";

const [chrome, baseUrl, outDir, ...cases] = process.argv.slice(2);

// SwiftShader keeps WebGL rasterization on the CPU: bit-exact for a
// fixed Chrome version on a fixed architecture.
const args = ["--use-angle=swiftshader", "--hide-scrollbars"];
// GitHub's Ubuntu runners forbid the unprivileged user namespaces
// Chrome's sandbox needs.
if (process.env.CI) args.push("--no-sandbox");
const browser = await puppeteer.launch({
  executablePath: chrome,
  headless: true,
  args,
});
let failed = false;
try {
  const page = await browser.newPage();
  await page.setViewport({ width: 1200, height: 750, deviceScaleFactor: 1 });
  const problems = [];
  page.on("pageerror", (e) => problems.push(`uncaught: ${e.message}`));
  page.on("console", (m) => {
    if (m.type() === "error") problems.push(`console.error: ${m.text()}`);
  });
  page.on("requestfailed", (r) => problems.push(`request failed: ${r.url()}`));

  for (const c of cases) {
    const sep = c.indexOf("|");
    const name = c.slice(0, sep);
    const query = c.slice(sep + 1);
    problems.length = 0;
    await page.goto(baseUrl + query, { waitUntil: "load" });
    // Poll from this side, not via waitForFunction: its rAF-based polling
    // starves in headless once the page goes quiet.
    const deadline = Date.now() + 30000;
    while (!(await page.evaluate("window.__historyflowReady === true"))) {
      const error = await page.evaluate("window.__historyflowError");
      if (error) problems.push(`page failed to start: ${error}`);
      if (problems.length || Date.now() > deadline) break;
      await new Promise((r) => setTimeout(r, 100));
    }
    if (!(await page.evaluate("window.__historyflowReady === true"))) {
      problems.push("never became ready");
    }
    if (problems.length) {
      failed = true;
      for (const p of problems) console.log(`error: ${name}: ${p}`);
      continue;
    }
    // Give the canvas commit a beat to reach the compositor.
    await new Promise((r) => setTimeout(r, 200));
    await page.screenshot({ path: `${outDir}/e2e-received-${name}.png` });
    console.log(`shot: ${name}`);
  }
} finally {
  await browser.close();
}
process.exit(failed ? 1 : 0);
