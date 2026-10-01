function showError(message) {
  const el = document.getElementById("error");
  el.hidden = !message;
  el.textContent = message || "";
}

function showSummary(text) {
  const empty = document.getElementById("empty");
  const summary = document.getElementById("summary");
  if (!text) {
    empty.hidden = false;
    summary.hidden = true;
    summary.textContent = "";
    return;
  }
  empty.hidden = true;
  summary.hidden = false;
  summary.textContent = text;
}

const bridge = window.__TAURI__ && window.__TAURI__.core;
if (!bridge || typeof bridge.invoke !== "function") {
  showError("Open this in the Exifguard window.");
}

async function call(cmd, args) {
  if (!bridge || typeof bridge.invoke !== "function") {
    showError("Open this in the Exifguard window.");
    throw new Error("Open this in the Exifguard window.");
  }
  try {
    return await bridge.invoke(cmd, args || {});
  } catch (err) {
    const message = typeof err === "string" ? err : (err && err.message) || String(err);
    showError(message);
    throw err;
  }
}

function codesOf(report) {
  const codes = (report.findings || []).map((finding) => finding.code).join(", ");
  return codes || "clean";
}

function renderRows(rows) {
  const list = document.getElementById("results");
  list.replaceChildren();
  if (!rows.length) {
    const empty = document.createElement("li");
    empty.textContent = "No JPEG files.";
    empty.style.cursor = "default";
    list.appendChild(empty);
    return;
  }
  const head = document.createElement("li");
  head.style.cursor = "default";
  const headNum = document.createElement("span");
  headNum.className = "num";
  headNum.textContent = "PATH";
  const headMeta = document.createElement("span");
  headMeta.className = "meta";
  headMeta.textContent = "FLAGS  CODES";
  head.append(headNum, headMeta);
  list.appendChild(head);
  for (const row of rows) {
    const li = document.createElement("li");
    li.dataset.path = row.path;
    if (row.error) li.dataset.error = row.error;
    const num = document.createElement("span");
    num.className = "num";
    num.textContent = row.path;
    const meta = document.createElement("span");
    meta.className = "meta";
    meta.textContent = row.flags + "  " + row.codes;
    li.append(num, meta);
    list.appendChild(li);
  }
}

function pathValue() {
  return document.getElementById("path").value.trim();
}

async function checkPath() {
  showError("");
  const path = pathValue();
  const result = await call("check_file", { path });
  renderRows([
    {
      path: result.report.path,
      flags: String((result.report.findings || []).length),
      codes: codesOf(result.report),
    },
  ]);
  showSummary(result.text);
  const row = document.querySelector("#results li[data-path]");
  if (row) row.className = "active";
}

async function scanPath() {
  showError("");
  const path = pathValue();
  const result = await call("scan_dir", { path });
  const rows = result.reports.map((report) => ({
    path: report.path,
    flags: String((report.findings || []).length),
    codes: codesOf(report),
  }));
  for (const error of result.errors || []) {
    rows.push({
      path: error.path,
      flags: "error",
      codes: error.message,
      error: error.message,
    });
  }
  rows.sort((a, b) => a.path.localeCompare(b.path));
  renderRows(rows);
  const notes = (result.errors || [])
    .map((error) => "warning: " + error.path + ": " + error.message)
    .join("\n");
  showSummary(notes ? result.text + "\n" + notes : result.text);
}

document.addEventListener("click", async (event) => {
  if (event.target.closest("#check-btn")) {
    await checkPath();
    return;
  }
  if (event.target.closest("#scan-btn")) {
    await scanPath();
    return;
  }
  const row = event.target.closest("#results li[data-path]");
  if (!row) return;
  document.querySelectorAll("#results li").forEach((li) => {
    li.classList.toggle("active", li === row);
  });
  if (row.dataset.error) {
    showSummary(row.dataset.path + "\n" + row.dataset.error);
    return;
  }
  const result = await call("check_file", { path: row.dataset.path });
  showSummary(result.text);
});

document.addEventListener("submit", (event) => {
  if (event.target.matches("#path-form")) event.preventDefault();
});
