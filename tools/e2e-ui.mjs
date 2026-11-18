// e2e behaviour tests: drives the real page in the pinned Chrome and
// checks what the screenshots cannot — that the crosshair, the text
// panel and the keyboard stay in step. Driven by tools/e2e.sh.
//
//   node tools/e2e-ui.mjs <chrome> <base-url> <command-url>
//
// <base-url> is the viewer on a plain web server; <command-url> is the
// historyflow command serving the small repository tools/e2e.sh makes.
import puppeteer from "puppeteer-core";

const [chrome, baseUrl, commandUrl] = process.argv.slice(2);

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

// Checks something that follows from an event the page handles in its own
// time (a scroll, an animation frame): looks until it is so, and fails
// only if it still is not after five seconds. Never a fixed wait — how
// long the page takes depends on the machine. Returns what it last saw.
async function eventually(what, look, ok) {
  const deadline = Date.now() + 5000;
  for (;;) {
    const got = await look();
    if (ok(got)) return got;
    if (Date.now() > deadline) {
      check(what, false, got);
      return got;
    }
    await pause(20);
  }
}

try {
  const page = await browser.newPage();
  await page.setViewport({ width: 1200, height: 750, deviceScaleFactor: 1 });
  page.on("pageerror", (e) => check(`no uncaught errors: ${e.message}`, false));
  // E2E_SLOW=6 runs the page as on a machine six times slower, to shake
  // out anything that depends on timing.
  if (process.env.E2E_SLOW)
    await page.emulateCPUThrottling(Number(process.env.E2E_SLOW));

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
  const scrollTo = (fraction) =>
    page.evaluate((fraction) => {
      const el = document.getElementById("text");
      el.scrollTop = (el.scrollHeight - el.clientHeight) * fraction;
    }, fraction);
  const end = await page.evaluate(
    () => window.__historyflow.detail().passages.at(-1)[4],
  );
  await scrollTo(1);
  at = await eventually(
    "scrolled to the end, the crosshair is at the version's end",
    cursor,
    (at) => near(at.y, end, 0.01),
  );
  check("scrolling keeps the version", at.version === 30, at);
  await scrollTo(0);
  await eventually(
    "scrolled to the start, the crosshair is at the top",
    cursor,
    (at) => near(at.y, 0, 0.01),
  );
  await scrollTo(0.5);
  await eventually(
    "scrolled half way, the crosshair is inside the version",
    cursor,
    (at) => at.y > 0.02 && at.y < end - 0.02,
  );
  await eventually(
    "after scrolling, the marker is still on the crosshair's passage",
    markerOnPassage,
    (link) => link.ok,
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
  // --- The scroll thumb appears when zoomed in, shows the part of the
  // history on screen, and can be dragged.
  const bar = () =>
    page.evaluate(() => {
      const track = document.getElementById("scroll");
      const t = track.getBoundingClientRect();
      const b = document.getElementById("thumb").getBoundingClientRect();
      return {
        hidden: track.hidden,
        left: b.left - t.left,
        width: b.width,
        track: t.width,
        x: t.left,
        y: t.top + 2,
      };
    });
  check("zoomed out, there is no scroll thumb", (await bar()).hidden);
  await wheel(12 + plotWidth / 2, { deltaY: -500 });
  seen = await view();
  let thumb = await bar();
  check("zoomed in, the scroll thumb appears", !thumb.hidden, thumb);
  check("the track spans the plot", near(thumb.track, plotWidth, 1), thumb);
  check(
    "the thumb is as wide as the share of history on screen",
    near(thumb.width, seen[1] * plotWidth, 1),
    thumb,
  );
  check(
    "the thumb starts where the view does",
    near(thumb.left, seen[0] * plotWidth, 1),
    thumb,
  );
  await page.mouse.move(thumb.x + thumb.left + thumb.width / 2, thumb.y);
  await page.mouse.down();
  await page.mouse.move(thumb.x + thumb.left + thumb.width / 2 + 100, thumb.y, {
    steps: 3,
  });
  await pause(60);
  let dragged = await view();
  check(
    "dragging the thumb pans by the same share of the history",
    near(dragged[0] - seen[0], 100 / plotWidth, 2e-3),
    dragged,
  );
  await page.mouse.move(thumb.x + 5000, thumb.y, { steps: 3 });
  await page.mouse.up();
  await pause(60);
  dragged = await view();
  check(
    "the thumb stops at the end of the track",
    near(dragged[0] + dragged[1], 1, 1e-5),
    dragged,
  );
  check(
    "dragging the thumb leaves the crosshair alone",
    (await cursor()).version === 0,
  );
  await page.mouse.click(thumb.x + 3, thumb.y);
  await pause(60);
  check(
    "a press on the track moves the thumb there",
    (await view())[0] === 0,
    await view(),
  );
  for (let i = 0; i < 5; i++) await wheel(12 + plotWidth / 2, { deltaY: 2000 });
  check("zoomed back out, the thumb goes away", (await bar()).hidden);

  await page.select("#spacing", "date");
  await pause(60);
  check(
    "changing the spacing keeps the whole history in view",
    (await view()).join() === "0,1",
  );

  // --- The height can fit what is on screen, and the scale can be
  // logarithmic; clicks land on the right text under either.
  const yScale = () => page.evaluate("window.__historyflow.yScale()");
  await open("?e2e&height=view&view=0,0.25");
  let scale = await yScale();
  check(
    "fitted to the view, the height is that of the longest version on screen",
    scale[0] > 0.1 && scale[0] < 0.6 && scale[1] === 0,
    scale,
  );
  await page.mouse.click(box.left + box.width - 13, box.top + box.height - 13);
  at = await cursor();
  check(
    "the longest version on screen reaches the bottom of the plot",
    near(at.y, scale[0], 0.01),
    { at, scale },
  );
  // The scale eases to a new height: through values in between, then
  // exactly there. Watches it all the way, however fast or slow the
  // frames come.
  const watchScale = async (until) => {
    const seen = [];
    const deadline = Date.now() + 5000;
    for (;;) {
      seen.push((await yScale())[0]);
      const moving = await page.evaluate("window.__historyflow.moving()");
      if ((until(seen.at(-1)) && !moving) || Date.now() > deadline) return seen;
      await pause(4);
    }
  };
  const between = (low, high) => (v) => v > low + 1e-4 && v < high - 1e-4;
  await page.select("#height", "all");
  let tops = await watchScale((top) => top === 1);
  check(
    "a change of height eases; it does not snap",
    tops.some(between(scale[0], 1)),
    tops,
  );
  check(
    "and comes to rest at the whole history's height",
    tops.at(-1) === 1,
    tops,
  );
  await page.select("#height", "view");
  tops = await watchScale((top) => near(top, scale[0], 1e-6));
  check("and eases back to the view's", tops.some(between(scale[0], 1)), tops);
  check("where it rests", near(tops.at(-1), scale[0], 1e-6), tops);
  await wheel(12 + plotWidth / 2, { deltaY: 3000 });
  tops = await watchScale((top) => top === 1);
  check(
    "zooming out eases the height as the view grows",
    tops.some(between(scale[0], 1)),
    tops,
  );
  check(
    "and rests at the longest version now in view",
    tops.at(-1) === 1,
    tops,
  );
  await open("?e2e&scale=log");
  scale = await yScale();
  check("the log scale is on", scale[0] === 1 && scale[1] > 1, scale);
  await page.mouse.click(box.left + box.width - 13, box.top + box.height / 2);
  at = await cursor();
  const expected = (Math.sqrt(1 + scale[1]) - 1) / scale[1];
  check(
    "on a log scale, half way down the plot is well short of half the text",
    near(at.y, expected, 0.01) && at.y < 0.3,
    { at, expected },
  );
  link = await markerOnPassage();
  check(
    "on a log scale, the marker is on the crosshair's passage",
    link.ok,
    link,
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
  // --- A Wikipedia history is fetched in batches and shown as it arrives.
  // The wiki is played by canned responses: no request leaves the page.
  const revision = (revid, user, timestamp, content, extra = {}) => ({
    revid,
    user,
    timestamp,
    comment: `edit ${revid}`,
    slots: { main: { content } },
    ...extra,
  });
  const response = (revisions, next) => ({
    ...(next ? { continue: { rvcontinue: next, continue: "||" } } : {}),
    query: { pages: [{ title: "Chocolate", revisions }] },
  });
  const batches = [
    response(
      [
        revision(1, "Ada", "2003-01-01T00:00:00Z", "Chocolate is a food."),
        revision(
          2,
          "203.0.113.7",
          "2003-01-02T00:00:00Z",
          "Chocolate is a sweet food.",
          { anon: true },
        ),
        revision(
          3,
          "Ada",
          "2003-01-03T00:00:00Z",
          "Chocolate is a sweet food. It melts.",
        ),
      ],
      "20030103|3",
    ),
    response([
      revision(
        4,
        "Brooke",
        "2003-01-04T00:00:00Z",
        "Chocolate is a sweet food. It melts. It is brown.",
      ),
      revision(
        5,
        "Ada",
        "2003-01-05T00:00:00Z",
        "Chocolate is a sweet brown food. It melts.",
      ),
    ]),
  ];
  // Opens the page with fetch replaced for the wiki's API, answering the
  // nth request with the nth of `answers`.
  const openWiki = async (query, answers) => {
    const wiki = await browser.newPage();
    await wiki.setViewport({ width: 1200, height: 750, deviceScaleFactor: 1 });
    await wiki.evaluateOnNewDocument((answers) => {
      const real = window.fetch;
      window.__wikiRequests = [];
      window.fetch = async (url, options) => {
        if (!String(url).includes("/w/api.php")) return real(url, options);
        window.__wikiRequests.push({
          url: String(url),
          headers: options?.headers,
        });
        const answer =
          answers[Math.min(window.__wikiRequests.length, answers.length) - 1];
        return new Response(JSON.stringify(answer), { status: 200 });
      };
    }, answers);
    await wiki.goto(baseUrl + query, { waitUntil: "load" });
    const deadline = Date.now() + 30000;
    const settled =
      "window.__historyflowReady === true && !window.__historyflow.loading()";
    while (!(await wiki.evaluate(settled))) {
      if (Date.now() > deadline) throw new Error(`timeout loading ${query}`);
      await pause(50);
    }
    return wiki;
  };

  let wiki = await openWiki("?e2e&wiki=Chocolate&limit=40", batches);
  let loaded = await wiki.evaluate("window.__historyflow.summary()");
  let requests = await wiki.evaluate("window.__wikiRequests");
  check("both batches of the article arrive", loaded.versions === 5, loaded);
  check("the title is the wiki's", loaded.title === "Chocolate", loaded);
  check("wiki text is compared by words", loaded.mode === "prose", loaded);
  check(
    "authors are counted, the anonymous one in white",
    JSON.stringify(
      loaded.authors.map((a) => [a.name, a.versions, a.anonymous]),
    ) ===
      JSON.stringify([
        ["Ada", 3, false],
        ["203.0.113.7", 1, true],
        ["Brooke", 1, false],
      ]) && loaded.authors[1].color === "#ffffff",
    loaded.authors,
  );
  check("one request per batch", requests.length === 2, requests);
  check(
    "the first request asks the English Wikipedia for the article from its start",
    requests[0].url.startsWith("https://en.wikipedia.org/w/api.php?") &&
      requests[0].url.includes("&titles=Chocolate&") &&
      requests[0].url.includes("&rvlimit=40&") &&
      requests[0].url.includes("&rvdir=newer&") &&
      requests[0].url.includes("&origin=*") &&
      !requests[0].url.includes("rvcontinue"),
    requests[0],
  );
  check(
    "the second continues from the first and asks only for what is left",
    requests[1].url.includes("&rvcontinue=20030103%7C3") &&
      requests[1].url.includes("&rvlimit=37&"),
    requests[1],
  );
  check(
    "requests say who is asking",
    /^historyflow\//.test(requests[0].headers["Api-User-Agent"]),
    requests[0].headers,
  );
  check(
    "the crosshair follows the newest revision",
    (await wiki.evaluate("window.__historyflow.cursor().version")) === 4 &&
      (await wiki.evaluate(
        () => document.getElementById("version-number").textContent,
      )) === "5 of 5",
  );
  check(
    "the newest revision's text is shown",
    (await wiki.evaluate(() => document.getElementById("text").textContent)) ===
      "Chocolate is a sweet brown food. It melts.",
  );
  check(
    "Stop goes away when the fetch ends",
    await wiki.evaluate(() => document.getElementById("stop").hidden),
  );
  // Saved, and loaded back from what was saved.
  const reloaded = await wiki.evaluate(() => {
    const app = window.__historyflowApp;
    const file = app.archive();
    const magic = new TextDecoder().decode(file.slice(0, 22));
    const summary = JSON.parse(app.load(file, ""));
    return {
      magic,
      bytes: file.length,
      summary,
      last: JSON.parse(app.version_detail(4)),
    };
  });
  check(
    "what was fetched can be saved as an archive and loaded back",
    reloaded.magic === "historyflow-archive/1\n" &&
      reloaded.summary.title === "Chocolate" &&
      reloaded.summary.versions === 5 &&
      reloaded.summary.mode === "prose" &&
      reloaded.last.passages.map((passage) => passage[0]).join("") ===
        "Chocolate is a sweet brown food. It melts.",
    reloaded,
  );
  const fromJson = await wiki.evaluate(() => {
    const bundle = {
      format: "historyflow-bundle/1",
      title: "From JSON",
      mode: "lines",
      versions: [
        { id: "1", author: "ada", timestamp: 5, comment: "", text: "a\nb\n" },
      ],
    };
    const file = new TextEncoder().encode(JSON.stringify(bundle));
    return JSON.parse(window.__historyflowApp.load(file, ""));
  });
  check(
    "a bundle's JSON still loads",
    fromJson.title === "From JSON" &&
      fromJson.versions === 1 &&
      fromJson.mode === "lines",
    fromJson,
  );
  await wiki.close();

  wiki = await openWiki(
    "?e2e&wiki=https://de.wikipedia.org/wiki/Schokolade&limit=2",
    batches,
  );
  loaded = await wiki.evaluate("window.__historyflow.summary()");
  requests = await wiki.evaluate("window.__wikiRequests");
  check(
    "the limit stops the fetch though more remains",
    loaded.versions === 3 && requests.length === 1,
    loaded.versions,
  );
  check(
    "an address chooses the wiki, and the limit the batch size",
    requests[0].url.startsWith("https://de.wikipedia.org/w/api.php?") &&
      requests[0].url.includes("&titles=Schokolade&") &&
      requests[0].url.includes("&rvlimit=2&"),
    requests[0],
  );
  await wiki.close();

  wiki = await openWiki("?e2e&wiki=Nonesuch", [
    { query: { pages: [{ title: "Nonesuch", missing: true }] } },
  ]);
  const said = await wiki.evaluate(
    () => document.getElementById("status").textContent,
  );
  check(
    "a missing article is reported",
    said.includes("there is no article called “Nonesuch”"),
    said,
  );
  await wiki.close();
  // --- The historyflow command: the same page, served with a repository
  // to browse. Any file's history is read from git when it is chosen.
  const repo = await browser.newPage();
  await repo.setViewport({ width: 1200, height: 750, deviceScaleFactor: 1 });
  repo.on("pageerror", (e) => check(`no uncaught errors: ${e.message}`, false));
  const outside = [];
  repo.on("request", (request) => {
    if (!request.url().startsWith(commandUrl)) outside.push(request.url());
  });
  const repoState = () =>
    repo.evaluate(() => ({
      title: document.getElementById("title").textContent,
      status: document.getElementById("status").textContent,
      files: [...document.querySelectorAll("#file-list li")].map(
        (item) => item.title,
      ),
      current: document.querySelector("#file-list li.current")?.title ?? null,
      versions: window.__historyflow.summary()?.versions,
      mode: window.__historyflow.summary()?.mode,
      authors: (window.__historyflow.summary()?.authors ?? []).map(
        (a) => `${a.name} ${a.versions}`,
      ),
      text: document.getElementById("text").textContent,
      address: location.search,
    }));
  const openRepo = async (query) => {
    await repo.goto(commandUrl + query, { waitUntil: "load" });
    await eventually(
      `${query} loads`,
      () => repo.evaluate("window.__historyflowReady === true"),
      Boolean,
    );
  };

  await openRepo("?e2e");
  let state = await repoState();
  check(
    "the repository's files are listed",
    state.files.join() === "notes.md,src/main.rs",
    state.files,
  );
  check(
    "nothing is shown until a file is chosen",
    state.versions === 0 && state.title === "e2e-repo",
    state,
  );
  check(
    "and the page says so",
    state.status === "2 files. Choose one to see its history.",
    state.status,
  );

  await repo.click('#file-list li[title="src/main.rs"]');
  state = await eventually(
    "choosing a file shows its history",
    repoState,
    (state) => state.versions === 3,
  );
  check(
    "titled with the repository and the file",
    state.title === "e2e-repo: src/main.rs",
    state.title,
  );
  check(
    "with its authors",
    state.authors.join() === "ada 2,brooke 1",
    state.authors,
  );
  check("compared by lines, as source", state.mode === "lines", state.mode);
  check(
    "and the newest version's text",
    state.text === "fn main() {\n    two();\n}\n\nfn two() {}\n",
    state.text,
  );
  check(
    "the chosen file is marked in the list",
    state.current === "src/main.rs",
    state.current,
  );
  check(
    "and named in the address, to link to",
    state.address === "?file=src%2Fmain.rs",
    state.address,
  );

  await repo.click('#file-list li[title="notes.md"]');
  state = await eventually(
    "choosing another file shows that one",
    repoState,
    (state) => state.title === "e2e-repo: notes.md",
  );
  check(
    "prose is compared by words",
    state.mode === "prose" && state.versions === 1,
    state,
  );

  await repo.type("#file-filter", "MAIN");
  state = await repoState();
  check(
    "the filter narrows the list, whatever the case",
    state.files.join() === "src/main.rs",
    state.files,
  );

  await openRepo("?e2e&file=src%2Fmain.rs");
  state = await eventually(
    "a link to a file opens it",
    repoState,
    (state) => state.versions === 3,
  );
  check(
    "with that file marked",
    state.current === "src/main.rs",
    state.current,
  );

  await openRepo("?e2e&file=nonesuch.rs");
  state = await eventually(
    "a file with no history is reported",
    repoState,
    (state) => state.status.includes("no history for nonesuch.rs"),
  );
  check(
    "nothing was fetched from anywhere but the command",
    outside.length === 0,
    outside,
  );
  await repo.close();
} catch (error) {
  failures += 1;
  console.log(`FAILED: ${error.message}`);
} finally {
  await browser.close();
}
process.exit(failures ? 1 : 0);
