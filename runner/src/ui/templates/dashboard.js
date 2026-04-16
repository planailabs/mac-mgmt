// Post one of the runner's JSON action endpoints and soft-refresh the
// page. Destructive ops prompt first.
async function runAction(path, confirmMsg, evt) {
  if (confirmMsg && !confirm(confirmMsg)) return;
  const btn = evt.currentTarget;
  const orig = btn.textContent;
  btn.disabled = true;
  btn.textContent = "…";
  try {
    const r = await fetch(path, { method: "POST" });
    const body = await r.json().catch(() => ({}));
    if (!r.ok || body.ok === false) {
      alert("Failed: " + r.status + " " + (body.detail || ""));
    }
  } catch (e) {
    alert("Request failed: " + e);
  }
  setTimeout(() => window.location.reload(), 400);
}

// Auto-refresh: server stays authoritative, no DOM diffing.
setTimeout(() => window.location.reload(), 5000);
