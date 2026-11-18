// Page glue: loads the wasm module, fetches or reads a bundle, and wires
// the controls, the plot's crosshair and the text panel together.
// Vanilla JS, no bundler; pkg/ is wasm-bindgen output.
//
// URL parameters, all optional:
//   src       bundle to load (default: the demo)
//   coloring  author | age
//   spacing   version | date
//   mode      lines | prose | sentences (default: the bundle's own)
//   at        version,y — where the crosshair starts (default: the last
//             version, at its top); y is a fraction of the plot's height
//   view      start,width — the part of the history on screen, in
//             fractions of the whole (default: all of it)
//   no-ui     hide everything but the plot (used by the e2e screenshots)
//   e2e       leave the build id out of the status line
import init, { App, version } from "./pkg/historyflow_web.js";

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
  mode: $("mode"),
};

let app;
// The text of the loaded bundle, kept so it can be re-read in another mode.
let bundle = "";
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
// Set when the panel is scrolled by the page itself, so the scroll event
// that follows is not mistaken for the reader scrolling.
let scrolledByPage = false;

function draw() {
  const ratio = window.devicePixelRatio || 1;
  canvas.width = Math.round(canvas.clientWidth * ratio);
  canvas.height = Math.round(canvas.clientHeight * ratio);
  app.draw(ratio);
}

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

// The colour of text that `origin` introduced, seen in `version`: the
// plot's colouring, but dimmed less with age so that old text stays
// readable.
function passageColor(author, origin, version) {
  const span = Math.max(1, summary.versions - 1);
  const aged = Math.min(1, (version - origin) / span);
  if (controls.coloring.value === "age") {
    const grey = Math.round(255 * (1 - 0.6 * aged));
    return `rgb(${grey} ${grey} ${grey})`;
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
  if (text.scrollTop !== before) scrolledByPage = true;
  marker.style.top = `${Math.round(offset - text.scrollTop)}px`;
}

function onTextScroll() {
  const range = scrollRange();
  const down = range > 0 ? (text.scrollTop / range) * text.clientHeight : 0;
  marker.style.top = `${Math.round(down)}px`;
  if (scrolledByPage) {
    scrolledByPage = false;
    return;
  }
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
  status.textContent =
    `${summary.versions} versions, ${day(summary.first_timestamp)} to ` +
    `${day(summary.last_timestamp)} · ${summary.authors.length} authors`;
  // The build differs on every commit, so it stays out of e2e screenshots.
  if (!params.has("e2e")) {
    const build = window.__historyflowBuild ?? { id: "unknown", date: "" };
    status.textContent += ` · history flow ${version()}, build ${build.id} ${build.date}`;
  }
}

// Loads bundle text, in the given mode or (if empty) the bundle's own,
// and puts the crosshair on version `at[0]` at y `at[1]`.
function load(source, mode, at) {
  summary = JSON.parse(app.load(source, mode));
  bundle = source;
  detail = null;
  showSummary();
  if (summary.versions > 0) select(at[0] ?? summary.versions - 1, at[1] ?? 0);
  else draw();
}

function report(error) {
  status.textContent = String(error);
  console.error(error);
}

async function readFile(file) {
  try {
    load(await file.text(), "", []);
  } catch (error) {
    report(error);
  }
}

try {
  await init();
  app = new App(canvas);
  for (const name of ["coloring", "spacing"]) {
    if (params.has(name)) controls[name].value = params.get(name);
  }
  app.set_coloring(controls.coloring.value);
  app.set_spacing(controls.spacing.value);

  const source = params.get("src") ?? "samples/demo.json";
  const response = await fetch(source);
  if (!response.ok) throw new Error(`${source}: HTTP ${response.status}`);
  const at = (params.get("at") ?? "").split(",").filter(Boolean).map(Number);
  load(await response.text(), params.get("mode") ?? "", at);
  if (params.has("view")) {
    app.set_view(...params.get("view").split(",").map(Number));
    draw();
  }

  controls.coloring.onchange = () => {
    app.set_coloring(controls.coloring.value);
    detail = null;
    select(cursor.version, cursor.y);
  };
  controls.spacing.onchange = () => {
    app.set_spacing(controls.spacing.value);
    draw();
  };
  controls.mode.onchange = () => {
    try {
      load(bundle, controls.mode.value, [cursor.version, cursor.y]);
    } catch (error) {
      report(error);
    }
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

  $("open").onclick = () => $("file").click();
  $("file").onchange = (event) => {
    if (event.target.files[0]) readFile(event.target.files[0]);
  };
  // A bundle can also be dropped anywhere on the page.
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
  window.__historyflow = {
    cursor: () => cursor,
    detail: () => detail,
    view: () => [...app.view()],
  };
  window.__historyflowReady = true;
} catch (error) {
  report(`failed to start: ${error}`);
  window.__historyflowError = String(error);
  throw error;
}
