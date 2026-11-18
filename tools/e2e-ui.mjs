// e2e behaviour tests: drives the real page in the pinned Chrome and
// checks what the screenshots cannot — that the crosshair, the text
// panel and the keyboard stay in step. Driven by tools/e2e.sh.
//
//   node tools/e2e-ui.mjs <chrome> <base-url>
import puppeteer from "puppeteer-core";

const [chrome, baseUrl] = process.argv.slice(2);

const args = ["--use-angle=swiftshader", "--hide-scrollbars"];
if (process.env.CI) args.push("--no-sandbox");
const browser = await puppeteer.launch({
  executablePath: chrome,
  headless: true,
  args,
});

let failures = 0;
function check(what, ok, got) {
  if (ok) return;
  failures += 1;
  console.log(
    `FAILED: ${what}${got === undefined ? "" : ` (got ${JSON.stringify(got)})`}`,
  );
}
const near = (a, b, tolerance) => Math.abs(a - b) <= tolerance;
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

try {
  const page = await browser.newPage();
  await page.setViewport({ width: 1200, height: 750, deviceScaleFactor: 1 });
  page.on("pageerror", (e) => check(`no uncaught errors: ${e.message}`, false));

  async function open(query) {
    await page.goto(baseUrl + query, { waitUntil: "load" });
    const deadline = Date.now() + 30000;
    while (!(await page.evaluate("window.__historyflowReady === true"))) {
      if (Date.now() > deadline) throw new Error(`timeout loading ${query}`);
      await pause(100);
    }
  }
  const cursor = () => page.evaluate("window.__historyflow.cursor()");
  const text = (id) =>
    page.evaluate((id) => document.getElementById(id).textContent, id);

  // Whether the panel's marker line lies on the passage that the
  // crosshair's y falls in — the link between plot and text. A y exactly
  // on the boundary of two passages belongs to both, and the marker may
  // be on either or in the gap between them.
  const markerOnPassage = () =>
    page.evaluate(() => {
      const { y } = window.__historyflow.cursor();
      // The passages' bounds arrive as decimal text and y as a float.
      const slack = 1e-6;
      const spans = document.getElementById("text").children;
      const boxes = window.__historyflow
        .detail()
        .passages.flatMap((passage, i) =>
          passage[3] <= y + slack && y - slack <= passage[4]
            ? [spans[i].getBoundingClientRect()]
            : [],
        );
      const top = Math.min(...boxes.map((box) => box.top));
      const bottom = Math.max(...boxes.map((box) => box.bottom));
      const marker = document
        .getElementById("marker")
        .getBoundingClientRect().top;
      return {
        ok: marker >= top - 1 && marker <= bottom + 1,
        marker,
        top,
        bottom,
        y,
      };
    });

  // --- Starting position, from the URL.
  await open("?e2e&at=30,0.35");
  let at = await cursor();
  check("at= selects the version", at.version === 30, at);
  check("at= sets the position", near(at.y, 0.35, 1e-6), at);
  check(
    "the panel names the version",
    (await text("version-number")) === "31 of 54",
  );
  let link = await markerOnPassage();
  check("the marker is on the passage under the crosshair", link.ok, link);

  // --- Scrolling the text moves the crosshair down the same version.
  const scrollTo = async (fraction) => {
    await page.evaluate((fraction) => {
      const el = document.getElementById("text");
      el.scrollTop = (el.scrollHeight - el.clientHeight) * fraction;
    }, fraction);
    await pause(100);
  };
  const end = await page.evaluate(
    () => window.__historyflow.detail().passages.at(-1)[4],
  );
  await scrollTo(1);
  at = await cursor();
  check(
    "scrolled to the end, the crosshair is at the version's end",
    near(at.y, end, 0.01),
    at,
  );
  check("scrolling keeps the version", at.version === 30, at);
  await scrollTo(0);
  at = await cursor();
  check(
    "scrolled to the start, the crosshair is at the top",
    near(at.y, 0, 0.01),
    at,
  );
  await scrollTo(0.5);
  link = await markerOnPassage();
  check(
    "after scrolling, the marker is still on the crosshair's passage",
    link.ok,
    link,
  );
  const middle = (await cursor()).y;
  check(
    "scrolled half way, the crosshair is inside the version",
    middle > 0.02 && middle < end - 0.02,
    middle,
  );

  // --- The keyboard steps through versions, keeping the position.
  await page.keyboard.press("ArrowLeft");
  at = await cursor();
  check("ArrowLeft selects the previous version", at.version === 29, at);
  check(
    "the panel follows the keyboard",
    (await text("version-number")) === "30 of 54",
  );
  link = await markerOnPassage();
  check(
    "after a step, the marker is on the crosshair's passage",
    link.ok,
    link,
  );
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("ArrowRight");
  check("ArrowRight selects the next version", (await cursor()).version === 31);

  // --- Clicking the plot picks the nearest version and clamps to its text.
  const box = await page.evaluate(() => {
    const r = document.getElementById("plot").getBoundingClientRect();
    return { left: r.left, top: r.top, width: r.width, height: r.height };
  });
  await page.mouse.click(box.left + 13, box.top + box.height - 13);
  at = await cursor();
  check(
    "a click at the left edge selects the first version",
    at.version === 0,
    at,
  );
  check(
    "a click below a version's text stops at its end",
    at.y > 0 && at.y < 0.2,
    at,
  );
  await page.mouse.click(box.left + box.width - 13, box.top + 13);
  at = await cursor();
  check(
    "a click at the right edge selects the last version",
    at.version === 53,
    at,
  );
  check("a click at the top is at the top", near(at.y, 0, 0.01), at);
  await page.mouse.click(box.left + box.width / 2, box.top + box.height / 4);
  at = await cursor();
  check(
    "a click in the middle selects a middle version",
    near(at.version, 26.5, 1),
    at,
  );
  link = await markerOnPassage();
  check(
    "after a click, the marker is on the crosshair's passage",
    link.ok,
    link,
  );

  // --- Dragging follows the pointer.
  await page.mouse.move(box.left + box.width * 0.25, box.top + 40);
  await page.mouse.down();
  await page.mouse.move(box.left + box.width * 0.75, box.top + 40, {
    steps: 4,
  });
  await page.mouse.up();
  check(
    "a drag ends on the version under the pointer",
    near((await cursor()).version, 39.75, 1),
  );

  // --- The wheel zooms time about the pointer, between the whole history
  // and three revisions; scrolling sideways pans.
  const view = () => page.evaluate("window.__historyflow.view()");
  const wheel = async (x, deltas) => {
    await page.mouse.move(box.left + x, box.top + 100);
    await page.mouse.wheel(deltas);
    await pause(60);
  };
  const plotWidth = box.width - 24;
  const three = 2 / 53; // three of 54 evenly spaced versions
  await open("?e2e");
  check(
    "the view starts as the whole history",
    (await view()).join() === "0,1",
  );
  await wheel(12 + plotWidth / 4, { deltaY: -200 });
  let seen = await view();
  check("wheel up zooms in", near(seen[1], Math.exp(-0.4), 1e-4), seen);
  check(
    "zooming holds what is under the pointer",
    near(seen[0] + seen[1] / 4, 0.25, 1e-4),
    seen,
  );
  await wheel(12 + plotWidth / 4, { deltaX: plotWidth / 4 });
  const panned = await view();
  check(
    "scrolling sideways pans by that many pixels",
    near(panned[0] - seen[0], seen[1] / 4, 1e-4),
    panned,
  );
  check("panning keeps the zoom", near(panned[1], seen[1], 1e-6), panned);
  for (let i = 0; i < 5; i++)
    await wheel(12 + plotWidth / 2, { deltaY: -2000 });
  seen = await view();
  check(
    "zooming in stops at three revisions",
    near(seen[1], three, 1e-5),
    seen,
  );
  await page.mouse.click(box.left + box.width / 2, box.top + 13);
  at = await cursor();
  check(
    "a click when zoomed selects the version under the pointer",
    near(at.version / 53, seen[0] + seen[1] / 2, 0.5 / 53 + 1e-4),
    at,
  );
  await wheel(12, { deltaX: 1e6 });
  seen = await view();
  check(
    "panning stops at the end of the history",
    near(seen[0] + seen[1], 1, 1e-5),
    seen,
  );
  for (let i = 0; i < 60; i++) await page.keyboard.press("ArrowLeft");
  seen = await view();
  check(
    "stepping to an off-screen version brings it into view",
    (await cursor()).version === 0 && seen[0] === 0,
    seen,
  );
  for (let i = 0; i < 5; i++) await wheel(12 + plotWidth / 2, { deltaY: 2000 });
  check(
    "zooming out stops at the whole history",
    (await view()).join() === "0,1",
  );
  await page.select("#spacing", "date");
  await pause(60);
  check(
    "changing the spacing keeps the whole history in view",
    (await view()).join() === "0,1",
  );

  // --- Comparing by lines shows the text as lines.
  await open("?e2e&at=40,0");
  await page.select("#mode", "lines");
  await pause(100);
  check(
    "line mode keeps lines whole",
    await page.evaluate(() =>
      document.getElementById("text").classList.contains("lines"),
    ),
  );
  check(
    "changing the comparison keeps the version",
    near((await cursor()).version, 39.75, 1),
  );
} catch (error) {
  failures += 1;
  console.log(`FAILED: ${error.message}`);
} finally {
  await browser.close();
}
process.exit(failures ? 1 : 0);
