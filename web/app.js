// Page glue: loads the wasm module, fetches or reads a bundle, and wires
// the controls to the plot. Vanilla JS, no bundler; pkg/ is wasm-bindgen
// output.
//
// URL parameters, all optional:
//   src       bundle to load (default: the demo)
//   coloring  author | age
//   spacing   version | date
//   mode      lines | prose | sentences (default: the bundle's own)
//   no-ui     hide everything but the plot (used by the e2e screenshots)
//   e2e       leave the build id out of the status line
import init, { App, version } from "./pkg/historyflow_web.js";

const params = new URLSearchParams(location.search);
document.body.classList.toggle("no-ui", params.has("no-ui"));

const $ = (id) => document.getElementById(id);
const canvas = $("plot");
const status = $("status");
const controls = {
  coloring: $("coloring"),
  spacing: $("spacing"),
  mode: $("mode"),
};

let app;
// The text of the loaded bundle, kept so it can be re-read in another mode.
let bundle = "";

function draw() {
  const ratio = window.devicePixelRatio || 1;
  canvas.width = Math.round(canvas.clientWidth * ratio);
  canvas.height = Math.round(canvas.clientHeight * ratio);
  app.draw(ratio);
}

function show(summary) {
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
  const build = window.__historyflowBuild ?? { id: "unknown", date: "" };
  status.textContent =
    `${summary.versions} versions, ${day(summary.first_timestamp)} to ` +
    `${day(summary.last_timestamp)} · ${summary.authors.length} authors`;
  // The build differs on every commit, so it stays out of e2e screenshots.
  if (!params.has("e2e")) {
    status.textContent += ` · history flow ${version()}, build ${build.id} ${build.date}`;
  }
}

// Loads bundle text, in the given mode or (if empty) the bundle's own.
function load(text, mode) {
  const summary = JSON.parse(app.load(text, mode));
  bundle = text;
  show(summary);
  draw();
}

function report(error) {
  status.textContent = String(error);
  console.error(error);
}

async function readFile(file) {
  try {
    load(await file.text(), "");
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
  load(await response.text(), params.get("mode") ?? "");

  controls.coloring.onchange = () => {
    app.set_coloring(controls.coloring.value);
    draw();
  };
  controls.spacing.onchange = () => {
    app.set_spacing(controls.spacing.value);
    draw();
  };
  controls.mode.onchange = () => {
    try {
      load(bundle, controls.mode.value);
    } catch (error) {
      report(error);
    }
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
  new ResizeObserver(draw).observe(canvas);

  // The e2e harness waits on this flag before capturing.
  window.__historyflowReady = true;
} catch (error) {
  report(`failed to start: ${error}`);
  window.__historyflowError = String(error);
  throw error;
}
