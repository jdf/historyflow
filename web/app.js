// Page glue: loads the wasm module, fetches or reads a history file, and wires
// the controls, the plot's crosshair and the text panel together.
// Vanilla JS, no bundler; pkg/ is wasm-bindgen output.
//
// URL parameters, all optional:
//   src       history file to load, an archive or a bundle (default: the
//             demo)
//   file      when served by the historyflow command on a repository: the
//             file whose history to show, as a path within it
//   wiki      instead of src: a Wikipedia article, by title or address,
//             whose history to fetch
//   limit     with wiki: how many revisions to fetch (default 500)
//   coloring  author | age
//   spacing   version | date
//   height    view | all — whether the plot is as tall as the longest
//             version of those on screen (the default), or of the whole
//             history
//   scale     linear | log
//   mode      lines | prose | sentences (default: the file's own)
//   at        version,y — where the crosshair starts (default: the last
//             version, at its top); y is a fraction of the plot's height
//   view      start,width — the part of the history on screen, in
//             fractions of the whole (default: all of it)
//   no-ui     hide everything but the plot (used by the e2e screenshots)
//   e2e       leave the build id out of the status line, and neither use
//             nor keep remembered settings
//
// The color, spacing, height and scale menus are remembered in this
// browser (localStorage) when changed, and start as they were left; a
// parameter above overrides what is remembered without replacing it.
// What to compare by is not remembered: each file suggests its own.
import init, {
  App,
  version,
  wikipedia_request,
  wikipedia_user_agent,
} from "./pkg/historyflow_web.js";

const params = new URLSearchParams(location.search);
document.body.classList.toggle("no-ui", params.has("no-ui"));

const $ = (id) => document.getElementById(id);
const canvas = $("plot");
const status = $("status");
const text = $("text");
const marker = $("marker");
const controls = {
  coloring: $("coloring"),
  spacing: $("spacing"),
  height: $("height"),
  scale: $("scale"),
  mode: $("mode"),
};

// The menus whose settings are remembered from one visit to the next.
const REMEMBERED = ["coloring", "spacing", "height", "scale"];
const remembering = !params.has("e2e");

// Storage can be unavailable or full (a private window, blocked site
// data); the page then works without remembering.
function remembered(name) {
  if (!remembering) return null;
  try {
    return localStorage.getItem(`historyflow.${name}`);
  } catch {
    return null;
  }
}

function remember(name) {
  if (!remembering) return;
  try {
    localStorage.setItem(`historyflow.${name}`, controls[name].value);
  } catch {
    // Not remembered, then.
  }
}

let app;
// The fetch of a Wikipedia history that is under way, if one is; setting
// its `stopped` ends it after the request in flight.
let loading = null;
// What app.load returned: title, mode, version count, authors.
let summary;
// What app.version_detail returned for the version in the text panel.
let detail;
// The crosshair: a version index, and y in the plot's units (0 is the top
// of the document, 1 the end of the longest version).
let cursor = { version: 0, y: 0 };
// Each passage's span in the text panel, and (measured when first needed)
// its top and height within the scrolled content.
let spans = [];
let boxes = null;
// Where the page itself last scrolled the panel to, until the scroll
// event for it arrives, so that event is not mistaken for the reader
// scrolling. A position, not a flag: if the reader scrolls before the
// event is delivered, one event reports both, and the position it reports
// is the reader's.
let pageScrolledTo = null;

function draw() {
  const ratio = window.devicePixelRatio || 1;
  canvas.width = Math.round(canvas.clientWidth * ratio);
  canvas.height = Math.round(canvas.clientHeight * ratio);
  if (stillness.matches) app.settle();
  // The vertical scale eases to where it is going; keep drawing until it
  // gets there.
  if (app.draw(ratio, performance.now())) scheduleDraw();
  showThumb();
}
const stillness = window.matchMedia("(prefers-reduced-motion: reduce)");

// --- The scroll thumb -----------------------------------------------------

// Under the plot, when zoomed in, a thumb shows which part of the history
// is on screen, and can be dragged; like any scrollbar's, it has a least
// size, so its travel rather than its edges maps to the view's.
const scroll = $("scroll");
const thumb = $("thumb");
const LEAST_THUMB = 16;

function thumbGeometry() {
  const [start, width] = app.view();
  const track = scroll.clientWidth;
  const size = Math.max(LEAST_THUMB, width * track);
  // How far the thumb can move, and how far the view can.
  return { start, width, size, travel: track - size, range: 1 - width };
}

function showThumb() {
  scroll.hidden = app.view()[1] > 0.9999;
  if (scroll.hidden) return;
  const { start, size, travel, range } = thumbGeometry();
  thumb.style.width = `${size}px`;
  thumb.style.left = `${range > 0 ? (start / range) * travel : 0}px`;
}

// Puts the thumb's left edge `left` pixels along the track.
function moveThumbTo(left) {
  const { width, travel, range } = thumbGeometry();
  if (travel <= 0) return;
  app.set_view((Math.max(0, Math.min(travel, left)) / travel) * range, width);
  scheduleDraw();
}

thumb.onpointerdown = (event) => {
  event.stopPropagation();
  thumb.setPointerCapture(event.pointerId);
  thumb.classList.add("held");
  const grabbed = event.clientX - thumb.getBoundingClientRect().left;
  const trackLeft = scroll.getBoundingClientRect().left;
  thumb.onpointermove = (move) =>
    moveThumbTo(move.clientX - trackLeft - grabbed);
  thumb.onpointerup = () => {
    thumb.classList.remove("held");
    thumb.onpointermove = thumb.onpointerup = null;
  };
};
// A press on the track beside the thumb centres the thumb there.
scroll.onpointerdown = (event) => {
  const { size } = thumbGeometry();
  moveThumbTo(event.clientX - scroll.getBoundingClientRect().left - size / 2);
};
scroll.addEventListener("wheel", (event) => onWheel(event), { passive: false });

// Draws once, before the next repaint, however many times it is asked:
// wheel events arrive faster than frames.
let drawScheduled = false;
function scheduleDraw() {
  if (drawScheduled) return;
  drawScheduled = true;
  requestAnimationFrame(() => {
    drawScheduled = false;
    draw();
  });
}

// The wheel and the trackpad move through time, the only axis that
// zooms. Scrolling sideways pans. Scrolling up and down zooms about the
// pointer, as does pinching (which browsers report as a wheel event with
// the control key down, in finer steps).
function onWheel(event) {
  event.preventDefault();
  const box = canvas.getBoundingClientRect();
  // Mice in some browsers report lines, not pixels.
  const scale = event.deltaMode === WheelEvent.DOM_DELTA_LINE ? 16 : 1;
  const [dx, dy] = [event.deltaX * scale, event.deltaY * scale];
  if (!event.ctrlKey && Math.abs(dx) > Math.abs(dy)) {
    app.pan(dx, box.width);
  } else {
    const rate = event.ctrlKey ? 0.01 : 0.002;
    app.zoom(Math.exp(dy * rate), event.clientX - box.left, box.width);
  }
  scheduleDraw();
}

// --- The text panel -------------------------------------------------------

// The color of text that `origin` introduced, seen in `version`: the
// plot's coloring, but dimmed less with age so that old text stays
// readable.
function passageColor(author, origin, version) {
  const span = Math.max(1, summary.versions - 1);
  const aged = Math.min(1, (version - origin) / span);
  if (controls.coloring.value === "age") {
    const gray = Math.round(255 * (1 - 0.6 * aged));
    return `rgb(${gray} ${gray} ${gray})`;
  }
  const hex = summary.authors[author].color;
  const dim = 1 - 0.4 * aged;
  const channel = (at) => Math.round(parseInt(hex.slice(at, at + 2), 16) * dim);
  return `rgb(${channel(1)} ${channel(3)} ${channel(5)})`;
}

function showVersion(index) {
  detail = JSON.parse(app.version_detail(index));
  spans = detail.passages.map(([passage, author, origin]) => {
    const span = document.createElement("span");
    span.textContent = passage;
    span.style.color = passageColor(author, origin, index);
    const by = summary.authors[author].name;
    span.title =
      origin === index ? `${by}, this version` : `${by}, version ${origin + 1}`;
    return span;
  });
  boxes = null;
  text.replaceChildren(...spans);
  text.classList.toggle("lines", summary.mode === "lines");

  const author = summary.authors[detail.author];
  $("version-number").textContent = `${index + 1} of ${summary.versions}`;
  $("version-id").textContent = detail.id.slice(0, 10);
  $("version-id").title = detail.id;
  $("version-swatch").style.background = author.color;
  $("version-author").textContent = author.name;
  const when = new Date(detail.timestamp * 1000).toISOString();
  $("version-date").textContent =
    `${when.slice(0, 10)} ${when.slice(11, 16)} UTC`;
  $("version-comment").textContent = detail.comment;
  $("previous").disabled = index === 0;
  $("next").disabled = index === summary.versions - 1;
  [...$("authors").children].forEach((item, id) => {
    item.classList.toggle("current", id === detail.author);
  });
}

// Each span's box within the scrolled content. The bounding rectangle
// rather than offsetTop/offsetHeight: it ends at the span's last line of
// text, where offsetHeight also counts the blank lines that follow a
// paragraph, and the marker would point into the gap.
function measure() {
  if (!boxes) {
    const origin = text.getBoundingClientRect().top - text.scrollTop;
    boxes = spans.map((span) => {
      const box = span.getBoundingClientRect();
      return { top: box.top - origin, height: box.height };
    });
  }
  return boxes;
}

// The index of the last entry of `list` whose `key` is at most `value`.
function lastAtMost(list, key, value) {
  let low = 0;
  let high = list.length - 1;
  while (low < high) {
    const middle = (low + high + 1) >> 1;
    if (key(list[middle]) <= value) low = middle;
    else high = middle - 1;
  }
  return low;
}

// From a y in the plot to a distance down the panel's content, and back.
// Within a passage both run in proportion.
function contentOffset(y) {
  if (spans.length === 0) return 0;
  const i = lastAtMost(detail.passages, (passage) => passage[3], y);
  const [, , , top, bottom] = detail.passages[i];
  const part = bottom > top ? Math.min(1, (y - top) / (bottom - top)) : 0;
  const box = measure()[i];
  return box.top + part * box.height;
}

function plotY(offset) {
  if (spans.length === 0) return 0;
  const i = lastAtMost(measure(), (box) => box.top, offset);
  const box = measure()[i];
  const part =
    box.height > 0 ? Math.min(1, (offset - box.top) / box.height) : 0;
  const [, , , top, bottom] = detail.passages[i];
  return top + part * (bottom - top);
}

// The panel's marker line shows where in the text the crosshair is. It
// slides from the panel's top to its bottom as the text scrolls from
// start to end (the way a scrollbar's thumb does), so that the very first
// and very last lines can both be reached; that makes the distance down
// the content that the marker points at proportional to the scroll.
function scrollRange() {
  return Math.max(0, text.scrollHeight - text.clientHeight);
}

function scrollTextTo(y) {
  const offset = contentOffset(y);
  const range = scrollRange();
  const before = text.scrollTop;
  text.scrollTop = range > 0 ? (offset * range) / text.scrollHeight : 0;
  if (text.scrollTop !== before) pageScrolledTo = text.scrollTop;
  marker.style.top = `${Math.round(offset - text.scrollTop)}px`;
}

function onTextScroll() {
  const range = scrollRange();
  const down = range > 0 ? (text.scrollTop / range) * text.clientHeight : 0;
  marker.style.top = `${Math.round(down)}px`;
  const byPage =
    pageScrolledTo !== null && Math.abs(text.scrollTop - pageScrolledTo) < 1;
  pageScrolledTo = null;
  if (byPage) return;
  if (range === 0) return;
  select(
    cursor.version,
    plotY((text.scrollTop * text.scrollHeight) / range),
    true,
  );
}

// --- The crosshair --------------------------------------------------------

// Moves the crosshair. `fromText` means the reader scrolled the text
// panel there, so the panel is already in the right place.
function select(index, y, fromText = false) {
  index = Math.max(0, Math.min(summary.versions - 1, index));
  cursor = { version: index, y: app.set_cursor(index, y) };
  if (detail?.index !== index) showVersion(index);
  if (!fromText && text.clientHeight > 0) scrollTextTo(cursor.y);
  draw();
}

function selectAtPointer(event) {
  const box = canvas.getBoundingClientRect();
  const picked = app.pick(
    event.clientX - box.left,
    event.clientY - box.top,
    box.width,
    box.height,
  );
  if (picked.length) select(picked[0], picked[1]);
}

// --- Loading --------------------------------------------------------------

function showSummary() {
  $("title").textContent = summary.title;
  document.title = `${summary.title} · history flow`;
  controls.mode.value = summary.mode;
  $("authors").replaceChildren(
    ...summary.authors.map((author) => {
      const item = document.createElement("li");
      const swatch = document.createElement("span");
      swatch.className = "swatch";
      swatch.style.background = author.color;
      const name = document.createElement("span");
      name.className = "name";
      name.textContent = author.name;
      name.title = author.name;
      const count = document.createElement("span");
      count.textContent = author.versions;
      item.append(swatch, name, count);
      return item;
    }),
  );
  const day = (seconds) => new Date(seconds * 1000).toISOString().slice(0, 10);
  status.textContent = summary.versions
    ? `${summary.versions} versions, ${day(summary.first_timestamp)} to ` +
      `${day(summary.last_timestamp)} · ${summary.authors.length} authors`
    : "No versions";
  if (loading)
    status.textContent = `Fetching from Wikipedia… ${status.textContent}`;
  $("stop").hidden = !loading;
  $("save").disabled = !summary.versions;
  // The build differs on every commit, so it stays out of e2e screenshots.
  if (!params.has("e2e")) {
    const build = window.__historyflowBuild ?? { id: "unknown", date: "" };
    status.textContent += ` · history flow ${version()}, build ${build.id} ${build.date}`;
  }
}

// Loads a history file's bytes, in the given mode or (if empty) the file's own,
// and puts the crosshair on version `at[0]` at y `at[1]`.
function load(source, mode, at) {
  if (loading) loading.stopped = true;
  loading = null;
  showLoaded(JSON.parse(app.load(source, mode)), at);
}

// Shows a history just loaded or rebuilt, with the crosshair on version
// `at[0]` at y `at[1]` (by default the last version, at its top).
function showLoaded(loaded, at) {
  summary = loaded;
  detail = null;
  showSummary();
  if (summary.versions > 0) {
    select(at[0] ?? summary.versions - 1, at[1] ?? 0);
  } else {
    text.replaceChildren();
    for (const id of ["number", "id", "author", "date", "comment"]) {
      $(`version-${id}`).textContent = "";
    }
    $("version-swatch").style.background = "none";
    draw();
  }
}

// --- Wikipedia ------------------------------------------------------------

// One request to the wiki, waiting and trying again when it says we are
// asking too fast.
async function fetchWikipedia(url, job) {
  for (let attempt = 0; ; attempt++) {
    // Wikimedia asks tools to say who they are; a page cannot set
    // User-Agent, so it reads this header instead.
    const response = await fetch(url, {
      headers: {
        // The command passes on the contact its user configured.
        "Api-User-Agent": wikipedia_user_agent(
          session?.wikipedia?.contact ?? "",
        ),
      },
    });
    if (response.ok) return response.text();
    if (response.status !== 429 || attempt === 4 || job.stopped) {
      throw new Error(`Wikipedia: HTTP ${response.status}`);
    }
    const wait =
      Number(response.headers.get("Retry-After")) || 5 * (attempt + 1);
    await new Promise((resolve) => setTimeout(resolve, wait * 1000));
  }
}

// Fetches an article's history from its first revision, up to `limit`
// revisions, a batch at a time, showing it as it arrives. The crosshair
// follows the newest revision unless it has been put somewhere else.
async function loadWikipedia(article, limit) {
  if (loading) loading.stopped = true;
  const job = (loading = { stopped: false });
  try {
    app.begin(article, "prose");
    showLoaded(JSON.parse(app.summary()), []);
    let next = "";
    let slow = false;
    while (!job.stopped && summary.versions < limit) {
      const url = wikipedia_request(article, next, limit - summary.versions);
      // Wikimedia asks that a request it took over a second to answer be
      // followed by five seconds' rest.
      if (slow) await new Promise((resolve) => setTimeout(resolve, 5000));
      if (job.stopped) break;
      const asked = performance.now();
      const response = await fetchWikipedia(url, job);
      slow = performance.now() - asked > 1000;
      if (job.stopped) break;
      const following = cursor.version >= summary.versions - 1;
      next = app.add_wikipedia(response);
      if (next === undefined) loading = null;
      summary = JSON.parse(app.summary());
      showSummary();
      if (following && summary.versions) {
        detail = null;
        select(summary.versions - 1, 0);
      } else {
        if (detail) {
          $("version-number").textContent =
            `${detail.index + 1} of ${summary.versions}`;
          $("next").disabled = false;
        }
        draw();
      }
      if (next === undefined) break;
    }
  } catch (error) {
    if (loading === job) loading = null;
    showSummary();
    report(error);
    return;
  }
  if (loading === job) loading = null;
  showSummary();
}

function report(error) {
  status.textContent = String(error);
  console.error(error);
}

// --- A repository, served by the historyflow command ----------------------

// What the command's server said it is showing, or null on a plain web
// host: { repository, files, open, history, wikipedia }.
let session = null;
// The file of the repository being shown.
let repoFile = null;
// No list this long is usable, or quick to build; filtering narrows it.
const MOST_FILES_LISTED = 500;
// The folders standing open, by path. All start closed.
const openFolders = new Set();
// While a filter is in force every folder with a match in it is shown
// open; these are the ones closed by hand since the filter last changed.
const closedInFilter = new Set();

const CHEVRON =
  '<svg class="chevron" viewBox="0 0 10 10" aria-hidden="true">' +
  '<path d="M3 1.5 7 5 3 8.5" /></svg>';
const FOLDER =
  '<svg class="icon" viewBox="0 0 16 16" aria-hidden="true">' +
  '<path d="M1.5 3.5h4.2l1.6 1.8h7.2v7.7h-13z" /></svg>';

const byName = new Intl.Collator(undefined, {
  numeric: true,
  sensitivity: "base",
});

// The files as a tree: { folders: Map(name → tree), files: [name] }, each
// in the order shown, folders before files.
function fileTree(paths) {
  const root = { folders: new Map(), files: [] };
  for (const path of paths) {
    const names = path.split("/");
    const file = names.pop();
    let tree = root;
    for (const name of names) {
      if (!tree.folders.has(name)) {
        tree.folders.set(name, { folders: new Map(), files: [] });
      }
      tree = tree.folders.get(name);
    }
    tree.files.push(file);
  }
  const sort = (tree) => {
    tree.folders = new Map(
      [...tree.folders].sort(([a], [b]) => byName.compare(a, b)),
    );
    tree.files.sort(byName.compare);
    tree.folders.forEach(sort);
  };
  sort(root);
  return root;
}

function showFiles() {
  const wanted = $("file-filter").value.toLowerCase();
  const filtering = wanted !== "";
  const matches = filtering
    ? session.files.filter((path) => path.toLowerCase().includes(wanted))
    : session.files;
  const rows = [];
  const row = (kind, path, name, depth) => {
    const item = document.createElement("li");
    item.className = kind;
    item.title = path;
    item.dataset.path = path;
    item.style.paddingLeft = `${4 + depth * 12}px`;
    const label = document.createElement("span");
    label.className = "name";
    label.textContent = name;
    if (kind === "folder") item.innerHTML = CHEVRON + FOLDER;
    item.append(label);
    rows.push(item);
    return item;
  };
  const list = (tree, prefix, depth) => {
    for (const [name, folder] of tree.folders) {
      const path = prefix + name;
      const open = filtering
        ? !closedInFilter.has(path)
        : openFolders.has(path);
      const item = row("folder", path, name, depth);
      item.setAttribute("aria-expanded", open);
      item.onclick = () => {
        const set = filtering ? closedInFilter : openFolders;
        // In one set membership means open, in the other closed.
        if (open === filtering) set.add(path);
        else set.delete(path);
        showFiles();
      };
      if (open) list(folder, `${path}/`, depth + 1);
    }
    for (const name of tree.files) {
      const path = prefix + name;
      const item = row("file", path, name, depth);
      item.classList.toggle("current", path === repoFile);
      item.onclick = () => openRepoFile(path);
    }
  };
  list(fileTree(matches), "", 0);
  const shown = rows.slice(0, MOST_FILES_LISTED);
  if (rows.length > MOST_FILES_LISTED) {
    const more = document.createElement("li");
    more.className = "more";
    more.textContent = `…and ${rows.length - MOST_FILES_LISTED} more; filter to narrow`;
    shown.push(more);
  } else if (rows.length === 0) {
    const none = document.createElement("li");
    none.className = "more";
    none.textContent = filtering ? "No file matches." : "No text files.";
    shown.push(none);
  }
  $("file-list").replaceChildren(...shown);
}

// Opens the folders a file is in, so that it is in the list.
function revealFile(path) {
  const names = path.split("/").slice(0, -1);
  names.forEach((_, i) => {
    const folder = names.slice(0, i + 1).join("/");
    openFolders.add(folder);
    closedInFilter.delete(folder);
  });
}

// Shows the history of one of the repository's files, which the server
// reads from git as it is asked.
async function openRepoFile(path) {
  repoFile = path;
  revealFile(path);
  showFiles();
  $("file-list")
    .querySelector(".current")
    ?.scrollIntoView({ block: "nearest" });
  history.replaceState(null, "", `?file=${encodeURIComponent(path)}`);
  status.textContent = `Reading the history of ${path}…`;
  try {
    const response = await fetch(
      `api/history?path=${encodeURIComponent(path)}`,
    );
    // The server explains a refusal in the body.
    if (!response.ok) throw new Error(await response.text());
    const file = new Uint8Array(await response.arrayBuffer());
    // Another file may have been chosen while this one was being read.
    if (repoFile === path) load(file, "", []);
  } catch (error) {
    report(error);
  }
}

async function startSession(address) {
  const response = await fetch(address);
  if (!response.ok) throw new Error(`${address}: HTTP ${response.status}`);
  session = await response.json();
  if (session.history) {
    const history = await fetch(session.history);
    if (!history.ok) throw new Error(await history.text());
    load(new Uint8Array(await history.arrayBuffer()), "", []);
    return;
  }
  if (session.wikipedia) {
    // Not awaited: the history shows as it arrives.
    loadWikipedia(session.wikipedia.article, session.wikipedia.revisions);
    return;
  }
  $("files").hidden = false;
  $("file-filter").oninput = () => {
    closedInFilter.clear();
    showFiles();
  };
  showFiles();
  const first = params.get("file") ?? session.open;
  if (first) {
    await openRepoFile(first);
  } else {
    app.begin(session.repository, "lines");
    showLoaded(JSON.parse(app.summary()), []);
    status.textContent = `${session.files.length} files. Choose one to see its history.`;
  }
}

async function readFile(file) {
  try {
    load(new Uint8Array(await file.arrayBuffer()), "", []);
  } catch (error) {
    report(error);
  }
}

try {
  await init();
  app = new App(canvas);
  for (const name of REMEMBERED) {
    // Anything but one of the menu's own choices would leave it blank.
    const known = (value) =>
      [...controls[name].options].some((option) => option.value === value);
    const wanted = [params.get(name), remembered(name)].find(known);
    if (wanted) controls[name].value = wanted;
  }
  app.set_coloring(controls.coloring.value);
  app.set_spacing(controls.spacing.value);
  app.set_height(controls.height.value);
  app.set_scale(controls.scale.value);

  // The historyflow command marks the page it serves, so that only then
  // does the page ask for a session; a plain web host has none to give.
  const server = document.querySelector('meta[name="historyflow-server"]');
  if (params.has("wiki")) {
    // Not awaited: the history shows as it arrives.
    loadWikipedia(params.get("wiki"), Number(params.get("limit")) || 500);
  } else if (server && !params.has("src")) {
    await startSession(server.content);
  } else {
    const source = params.get("src") ?? "samples/demo.hflow";
    const response = await fetch(source);
    if (!response.ok) throw new Error(`${source}: HTTP ${response.status}`);
    const at = (params.get("at") ?? "").split(",").filter(Boolean).map(Number);
    const file = new Uint8Array(await response.arrayBuffer());
    load(file, params.get("mode") ?? "", at);
    if (params.has("view")) {
      app.set_view(...params.get("view").split(",").map(Number));
      // A link opens at rest on its view, not easing into it.
      app.settle();
      draw();
    }
  }

  controls.coloring.onchange = () => {
    app.set_coloring(controls.coloring.value);
    remember("coloring");
    detail = null;
    select(cursor.version, cursor.y);
  };
  controls.spacing.onchange = () => {
    app.set_spacing(controls.spacing.value);
    remember("spacing");
    draw();
  };
  controls.height.onchange = () => {
    app.set_height(controls.height.value);
    remember("height");
    draw();
  };
  controls.scale.onchange = () => {
    app.set_scale(controls.scale.value);
    remember("scale");
    draw();
  };
  controls.mode.onchange = () => {
    // A fetch under way carries on, in the new mode.
    const rebuilt = JSON.parse(app.set_mode(controls.mode.value));
    showLoaded(rebuilt, [cursor.version, cursor.y]);
  };

  canvas.onpointerdown = (event) => {
    canvas.setPointerCapture(event.pointerId);
    selectAtPointer(event);
  };
  canvas.onpointermove = (event) => {
    if (event.buttons) selectAtPointer(event);
  };
  text.onscroll = onTextScroll;
  canvas.addEventListener("wheel", onWheel, { passive: false });
  // Stepping to a version that is off screen brings it on.
  const step = (by) => {
    const to = Math.max(0, Math.min(summary.versions - 1, cursor.version + by));
    app.reveal(to);
    select(to, cursor.y);
  };
  $("previous").onclick = () => step(-1);
  $("next").onclick = () => step(1);
  window.onkeydown = (event) => {
    const by = { ArrowLeft: -1, ArrowRight: 1 }[event.key];
    if (!by || event.target.matches("select, input") || !summary?.versions)
      return;
    event.preventDefault();
    step(by);
  };

  $("wikipedia").onclick = () => $("wiki").showModal();
  $("wiki").onclose = () => {
    if ($("wiki").returnValue !== "load") return;
    loadWikipedia(
      $("wiki-article").value,
      Number($("wiki-limit").value) || 500,
    );
  };
  $("stop").onclick = () => {
    if (loading) loading.stopped = true;
    loading = null;
    showSummary();
  };
  // Saves what is loaded as an archive, so a history fetched once need
  // not be fetched again.
  $("save").onclick = () => {
    const link = document.createElement("a");
    link.href = URL.createObjectURL(
      new Blob([app.archive()], { type: "application/octet-stream" }),
    );
    link.download = `${summary.title.replace(/[^\p{L}\p{N}]+/gu, "-")}.hflow`;
    link.click();
    URL.revokeObjectURL(link.href);
  };
  $("open").onclick = () => $("file").click();
  $("file").onchange = (event) => {
    if (event.target.files[0]) readFile(event.target.files[0]);
  };
  // A history file can also be dropped anywhere on the page.
  window.ondragover = (event) => {
    event.preventDefault();
    document.body.classList.add("dragging");
  };
  window.ondragleave = () => document.body.classList.remove("dragging");
  window.ondrop = (event) => {
    event.preventDefault();
    document.body.classList.remove("dragging");
    if (event.dataTransfer.files[0]) readFile(event.dataTransfer.files[0]);
  };
  new ResizeObserver(() => {
    boxes = null;
    if (summary?.versions && text.clientHeight > 0) scrollTextTo(cursor.y);
    draw();
  }).observe(canvas);

  // For the e2e tests: the state they assert on, and the flag they wait
  // for before looking.
  window.__historyflowApp = app;
  window.__historyflow = {
    cursor: () => cursor,
    detail: () => detail,
    view: () => [...app.view()],
    yScale: () => [...app.y_scale_now()],
    moving: () => drawScheduled,
    summary: () => summary,
    session: () => session,
    loading: () => loading !== null,
  };
  window.__historyflowReady = true;
} catch (error) {
  report(`failed to start: ${error}`);
  window.__historyflowError = String(error);
  throw error;
}
