// Page glue: loads the wasm module, sizes the canvas, and asks the
// renderer to draw. Vanilla JS, no bundler; pkg/ is wasm-bindgen output.
import init, { Renderer, version } from "./pkg/historyflow_web.js";

const params = new URLSearchParams(location.search);
// ?no-ui hides the chrome so e2e screenshots contain only the plot.
document.body.classList.toggle("no-ui", params.has("no-ui"));

const canvas = document.getElementById("plot");
const status = document.getElementById("status");

function resize() {
  const dpr = window.devicePixelRatio || 1;
  canvas.width = Math.round(canvas.clientWidth * dpr);
  canvas.height = Math.round(canvas.clientHeight * dpr);
}

try {
  await init();
  const renderer = new Renderer(canvas);
  const draw = () => {
    resize();
    renderer.draw();
  };
  new ResizeObserver(draw).observe(canvas);
  draw();
  const build = window.__historyflowBuild ?? { id: "unknown", date: "" };
  status.textContent = `history flow ${version()} · build ${build.id} ${build.date}`;
  // The e2e harness waits on this flag before capturing.
  window.__historyflowReady = true;
} catch (error) {
  status.textContent = `failed to start: ${error}`;
  window.__historyflowError = String(error);
  throw error;
}
