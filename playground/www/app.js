// The whole playground is one module with no dependencies: the compiler is the
// same Rust code the CLI uses, built to WebAssembly, so the page only has to
// move text in and pictures out.

import init, { compile, example_names, example_source } from "./pkg/qrab_playground.js";

const source = document.getElementById("source");
const gutter = document.getElementById("gutter");
const status = document.getElementById("status");
const preview = document.getElementById("preview");
const examples = document.getElementById("examples");
const target = document.getElementById("target");
const outputTitle = document.getElementById("output-title");

const EXTENSIONS = { svg: "svg", latex: "tex", typst: "typ" };
const TITLES = { svg: "Diagram", latex: "LaTeX / TikZ", typst: "Typst / Quill" };

let latest = { output: "", target: "svg" };
let zoom = 1;

await init();

// Source restoration ----------------------------------------------------
//
// A shared link carries the whole circuit in the URL fragment, so nothing is
// stored server-side and a link keeps working offline.

// Names arrive as "group/name"; each group becomes an <optgroup> so the ported
// qpic corpus does not bury the introductory circuits.
const groups = new Map();
for (const entry of example_names()) {
  const [group, name] = splitEntry(entry);
  if (!groups.has(group)) {
    const optgroup = document.createElement("optgroup");
    optgroup.label = group;
    examples.append(optgroup);
    groups.set(group, optgroup);
  }
  const option = document.createElement("option");
  option.value = entry;
  option.textContent = name;
  groups.get(group).append(option);
}

function splitEntry(entry) {
  const slash = entry.indexOf("/");
  return [entry.slice(0, slash), entry.slice(slash + 1)];
}

source.value = decodeFragment() ?? example_source(examples.value);
render();

function decodeFragment() {
  const fragment = location.hash.slice(1);
  if (!fragment) return null;
  try {
    const binary = atob(fragment.replaceAll("-", "+").replaceAll("_", "/"));
    const bytes = Uint8Array.from(binary, (character) => character.charCodeAt(0));
    return new TextDecoder().decode(bytes);
  } catch {
    return null;
  }
}

function toBase64(text) {
  const bytes = new TextEncoder().encode(text);
  const binary = Array.from(bytes, (byte) => String.fromCharCode(byte)).join("");
  return btoa(binary);
}

// URL fragments use the base64url alphabet so a shared link needs no escaping.
function encodeFragment(text) {
  return toBase64(text).replaceAll("+", "-").replaceAll("/", "_").replaceAll("=", "");
}

// Compile loop ----------------------------------------------------------
//
// Compiling is fast enough to run on every keystroke, but debouncing keeps the
// preview from flickering mid-word.

let pending = 0;
source.addEventListener("input", () => {
  clearTimeout(pending);
  pending = setTimeout(render, 120);
});
target.addEventListener("change", render);
examples.addEventListener("change", () => {
  source.value = example_source(examples.value);
  history.replaceState(null, "", location.pathname);
  render();
});

function render() {
  const kind = target.value;
  const result = compile(source.value, kind);
  latest = { output: result.output, target: kind };
  outputTitle.textContent = TITLES[kind];
  drawGutter(result.message ? result.line : 0);

  if (result.message) {
    status.classList.add("failed");
    status.textContent = [
      `${result.line}:${result.column}: ${result.message}`,
      result.help && `help: ${result.help}`,
      result.related,
    ]
      .filter(Boolean)
      .join("\n");
    preview.replaceChildren(element("p", "empty", "No output — fix the error to see the circuit."));
    return;
  }

  status.classList.remove("failed");
  status.textContent = result.summary;
  preview.replaceChildren(kind === "svg" ? diagram(result.output) : element("pre", "", result.output));
}

// The diagram goes into an <img>, not inline, so the generated markup can never
// become part of this document: a shared link is inert no matter what it holds.
function diagram(svg) {
  const image = document.createElement("img");
  image.src = `data:image/svg+xml;base64,${toBase64(svg)}`;
  image.alt = "Rendered quantum circuit";
  applyZoom(image);
  return image;
}

function element(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  node.textContent = text;
  return node;
}

// Gutter ----------------------------------------------------------------

function drawGutter(errorLine) {
  const lines = source.value.split("\n").length;
  gutter.replaceChildren();
  for (let line = 1; line <= lines; line += 1) {
    const node = element("div", line === errorLine ? "current" : "", String(line));
    gutter.append(node);
  }
  gutter.scrollTop = source.scrollTop;
}

source.addEventListener("scroll", () => {
  gutter.scrollTop = source.scrollTop;
});

// Tab indents instead of leaving the editor; the language uses two spaces.
source.addEventListener("keydown", (event) => {
  if (event.key !== "Tab") return;
  event.preventDefault();
  const { selectionStart, selectionEnd, value } = source;
  source.value = `${value.slice(0, selectionStart)}  ${value.slice(selectionEnd)}`;
  source.selectionStart = source.selectionEnd = selectionStart + 2;
  render();
});

// Toolbar ---------------------------------------------------------------

document.getElementById("share").addEventListener("click", async (event) => {
  const url = `${location.origin}${location.pathname}#${encodeFragment(source.value)}`;
  history.replaceState(null, "", url);
  try {
    await navigator.clipboard.writeText(url);
    flash(event.target, "Copied");
  } catch {
    flash(event.target, "Link in address bar");
  }
});

document.getElementById("download").addEventListener("click", () => {
  if (!latest.output) return;
  const blob = new Blob([latest.output], { type: "text/plain;charset=utf-8" });
  const link = document.createElement("a");
  link.href = URL.createObjectURL(blob);
  link.download = `circuit.${EXTENSIONS[latest.target]}`;
  link.click();
  URL.revokeObjectURL(link.href);
});

function flash(button, message) {
  const original = button.textContent;
  button.textContent = message;
  setTimeout(() => {
    button.textContent = original;
  }, 1200);
}

// Zoom ------------------------------------------------------------------

document.getElementById("zoom").addEventListener("click", (event) => {
  const action = event.target.dataset.zoom;
  if (!action) return;
  if (action === "fit") zoom = 1;
  if (action === "in") zoom = Math.min(zoom * 1.25, 8);
  if (action === "out") zoom = Math.max(zoom / 1.25, 0.2);
  const image = preview.querySelector("img");
  if (image) applyZoom(image);
});

function applyZoom(image) {
  if (zoom === 1) {
    image.style.width = "";
    image.style.maxWidth = "100%";
  } else {
    // Above 1x the image is allowed to overflow so the pane can scroll to it.
    image.style.maxWidth = "none";
    image.style.width = `${zoom * 100}%`;
  }
}

// Splitter --------------------------------------------------------------

const panes = document.getElementById("panes");
document.getElementById("splitter").addEventListener("pointerdown", (event) => {
  event.target.setPointerCapture(event.pointerId);
  const move = (moved) => {
    const box = panes.getBoundingClientRect();
    const vertical = window.matchMedia("(max-width: 720px)").matches;
    const fraction = vertical
      ? (moved.clientY - box.top) / box.height
      : (moved.clientX - box.left) / box.width;
    const percent = Math.min(Math.max(fraction, 0.15), 0.85) * 100;
    panes.style[vertical ? "gridTemplateRows" : "gridTemplateColumns"] =
      `${percent}% 6px ${100 - percent}%`;
  };
  const stop = () => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", stop);
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", stop);
});
