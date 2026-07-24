#!/usr/bin/env node
// Regenerate the language-specific screenshots embedded in the step-by-step
// guides under server/docs/en/ and server/docs/de/. Drives the running dev app
// over the Chrome DevTools Protocol — no extra npm deps.
//
// Ported from wg-vpng's server/scripts/regen-tutorial-screenshots.mjs,
// including the numbered step-marker overlay.
//
// Prereqs:
//   • the dev app is running and reachable (default http://localhost:8080)
//     with DEV_ONLY_NO_AUTH=1. Use a scratch database (CONFIG_PATH pointing at
//     a config with its own database.url) so no real fleet data ends up in
//     committed screenshots;
//   • `google-chrome` (or set CHROME=/path/to/chrome) is on PATH.
//
// Usage:
//   node server/scripts/regen-docs-screenshots.mjs
//   APP_URL=http://localhost:8080 CHROME=chromium node server/scripts/regen-docs-screenshots.mjs
//
// Writes <shot>-{en,de}.png into server/public/docs-img/, referenced from the
// docs as /docs-img/<shot>-<lang>.png:
//   clusters-list, cluster-new        → getting-started / cluster-setup
//   users-list, user-new              → user-management
//   admin-tokens                      → tokens-and-api
//   docs-list                         → getting-started

import { spawn, execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const APP = process.env.APP_URL || 'http://localhost:8080';
const CHROME = process.env.CHROME || 'google-chrome';
const OUT = join(dirname(fileURLToPath(import.meta.url)), '..', 'public', 'docs-img');
const PORT = 9457;
const sleep = ms => new Promise(r => setTimeout(r, ms));

// Per-language UI text (from server/src/web/{en-US,de-DE}.ftl) — the app is
// translated, so element lookup must use the active language's labels.
const LANGS = {
  en: {
    tag: 'en-US',
    create: 'Create', newCluster: 'New Cluster', newUser: 'New User',
    createToken: 'Create Admin Token', clustersTitle: 'Clusters',
    usersTitle: 'Users', docsTitle: 'Documentation',
    emailPlaceholder: 'user@example.com',
  },
  de: {
    tag: 'de-DE',
    create: 'Erstellen', newCluster: 'Neuer Cluster', newUser: 'Neuer Benutzer',
    createToken: 'Admin Token erstellen', clustersTitle: 'Cluster',
    usersTitle: 'Benutzer', docsTitle: 'Dokumentation',
    emailPlaceholder: 'benutzer@beispiel.de',
  },
};
const VIEWPORT = { width: 1280, height: 850, mobile: false };
const DEMO_CLUSTER = 'demo-macbook';
const DEMO_USER = { email: 'jane@example.com', name: 'Jane Doe' };

mkdirSync(OUT, { recursive: true });

// Seed the demo cluster straight into the scratch database — submitting the
// form from CDP is racy (synthetic input events don't reliably reach the
// Dioxus signal), and the form screenshot only needs the field filled.
// SHOTS_DB names the scratch psql database (matching CONFIG_PATH's
// database.url); set SHOTS_DB= (empty) to skip if the row already exists.
const DB = process.env.SHOTS_DB ?? 'mac_mgmt_shots';
if (DB) {
  execFileSync('psql', ['-d', DB, '-c',
    `INSERT INTO clusters (name) VALUES ('${DEMO_CLUSTER}') ON CONFLICT (name) DO NOTHING`]);
}
const profile = mkdtempSync(join(tmpdir(), 'mgmt-shots-'));
const chrome = spawn(CHROME, [
  '--headless=new', '--disable-gpu', '--hide-scrollbars', '--no-first-run',
  '--no-default-browser-check', `--remote-debugging-port=${PORT}`,
  `--user-data-dir=${profile}`, 'about:blank',
], { stdio: 'ignore' });

async function cdp() {
  for (let i = 0; i < 40; i++) {
    try { return await (await fetch(`http://localhost:${PORT}/json/version`)).json(); }
    catch { await sleep(250); }
  }
  throw new Error('chrome CDP did not come up');
}

const ver = await cdp();
const ws = new WebSocket(ver.webSocketDebuggerUrl);
await new Promise((res, rej) => { ws.onopen = res; ws.onerror = rej; });

let id = 0;
const pending = new Map();
ws.onmessage = m => {
  const x = JSON.parse(m.data);
  if (x.id && pending.has(x.id)) {
    const p = pending.get(x.id); pending.delete(x.id);
    x.error ? p.reject(new Error(JSON.stringify(x.error))) : p.resolve(x.result);
  }
};
const send = (method, params = {}, sessionId) =>
  new Promise((resolve, reject) => { const mid = ++id; pending.set(mid, { resolve, reject }); ws.send(JSON.stringify({ id: mid, method, params, sessionId })); });

const { targetId } = await send('Target.createTarget', { url: 'about:blank' });
const { sessionId } = await send('Target.attachToTarget', { targetId, flatten: true });
const S = (m, p) => send(m, p, sessionId);
await S('Page.enable'); await S('Runtime.enable');

async function evalJS(expression) {
  const r = await S('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
  if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description || 'eval error');
  return r.result.value;
}
async function waitFor(expr, timeout = 30000) {
  const t0 = Date.now();
  while (Date.now() - t0 < timeout) { if (await evalJS(expr)) return; await sleep(200); }
  throw new Error('timeout: ' + expr);
}
const byText = (tag, txt) => `[...document.querySelectorAll('${tag}')].find(e=>e.textContent.trim()===${JSON.stringify(txt)})`;
const clickByText = (tag, txt) => evalJS(`(()=>{const b=${byText(tag, txt)};if(b){b.click();return true}return false})()`);
async function shot(file) {
  const { data } = await S('Page.captureScreenshot', { format: 'png' });
  writeFileSync(join(OUT, file), Buffer.from(data, 'base64'));
  console.log('wrote', file);
}

// Numbered step markers (orange badges) overlaid on a target element before a
// shot. `elExpr` is a JS expression evaluating to the element (or null → skip).
async function mark(elExpr, label) {
  return evalJS(`(()=>{const el=(${elExpr}); if(!el) return false;
    const r=el.getBoundingClientRect();
    const b=document.createElement('div'); b.className='__docmark'; b.textContent=${JSON.stringify(String(label))};
    Object.assign(b.style,{position:'fixed',left:(r.left-16)+'px',top:(r.top-16)+'px',width:'30px',height:'30px',
      borderRadius:'9999px',background:'#e8552b',color:'#fff',display:'flex',alignItems:'center',justifyContent:'center',
      font:'700 16px system-ui,sans-serif',boxShadow:'0 2px 8px rgba(0,0,0,.4)',zIndex:'99999',pointerEvents:'none',border:'2px solid #fff'});
    document.body.appendChild(b); return true;})()`);
}
const clearMarks = () => evalJS(`document.querySelectorAll('.__docmark').forEach(e=>e.remove())`);

// Force the language before the app loads: the app restores it from
// localStorage['lang'] on startup (see server/src/web/app.rs), so an init
// script that seeds localStorage wins regardless of the host locale.
let langScriptId = null;
async function useLang(L) {
  if (langScriptId) {
    await S('Page.removeScriptToEvaluateOnNewDocument', { identifier: langScriptId }).catch(() => {});
  }
  const src = `try{localStorage.setItem('lang','${L.tag}');}catch(e){}`
    + `Object.defineProperty(navigator,'language',{get:()=>'${L.tag}',configurable:true});`
    + `Object.defineProperty(navigator,'languages',{get:()=>['${L.tag}'],configurable:true});`;
  langScriptId = (await S('Page.addScriptToEvaluateOnNewDocument', { source: src })).identifier;
  await S('Emulation.setDeviceMetricsOverride', { ...VIEWPORT, deviceScaleFactor: 2, screenWidth: VIEWPORT.width, screenHeight: VIEWPORT.height });
}

// Navigate and wait for hydration (the wasm-loading banner removes itself)
// plus a language-specific text so we know the locale has been applied.
async function goto(path, readyExpr) {
  await S('Page.navigate', { url: APP + path });
  await waitFor(`!document.getElementById('wasm-loading')`);
  await waitFor(readyExpr);
  await sleep(500);
  await evalJS('window.scrollTo(0,0)');
}

// Type into an input: set the value through the native setter and dispatch a
// bubbling `input` event so Dioxus's delegated oninput handler updates the
// signal (CDP Input.insertText updates the DOM but the signal stayed empty).
async function type(selExpr, text) {
  await evalJS(`(()=>{const el=(${selExpr}); el.focus();
    const set=Object.getOwnPropertyDescriptor(Object.getPrototypeOf(el),'value').set;
    set.call(el,${JSON.stringify(text)});
    el.dispatchEvent(new Event('input',{bubbles:true}));
    return el.value;})()`);
  await sleep(200);
}

async function capture(langKey) {
  const L = LANGS[langKey];
  await useLang(L);

  // ── Cluster creation (cluster-setup / getting-started) ──────────────
  await goto('/clusters/new', byText('button', L.create));
  const clusterNameInput = `document.querySelector('form input')`;
  await type(clusterNameInput, DEMO_CLUSTER);
  await clearMarks();
  await mark(clusterNameInput, '1');
  await mark(byText('button', L.create), '2');
  await sleep(150);
  await shot(`cluster-new-${langKey}.png`);
  await clearMarks();

  // ── Cluster list (getting-started) ──────────────────────────────────
  await goto('/', `document.body.innerText.includes(${JSON.stringify(DEMO_CLUSTER)})`);
  await clearMarks();
  await mark(`[...document.querySelectorAll('a')].find(a=>a.textContent.includes(${JSON.stringify(DEMO_CLUSTER)}))`, '1');
  await mark(byText('a', L.newCluster), '2');
  await sleep(150);
  await shot(`clusters-list-${langKey}.png`);
  await clearMarks();

  // ── User list (user-management) ─────────────────────────────────────
  await goto('/users', byText('a', L.newUser));
  await clearMarks();
  await mark(byText('a', L.newUser), '1');
  await sleep(150);
  await shot(`users-list-${langKey}.png`);
  await clearMarks();

  // ── New user form (user-management) ─────────────────────────────────
  await goto('/users/new', byText('button', L.create));
  const emailInput = `document.querySelector('input[placeholder=${JSON.stringify(L.emailPlaceholder)}]')`;
  const nameInput = `[...document.querySelectorAll('form input[type=text],form input:not([type])')].filter(i=>i.placeholder!==${JSON.stringify(L.emailPlaceholder)})[0]`;
  await type(emailInput, DEMO_USER.email);
  await type(nameInput, DEMO_USER.name);
  await clearMarks();
  await mark(emailInput, '1');
  await mark(nameInput, '2');
  await mark(byText('button', L.create), '3');
  await sleep(150);
  await shot(`user-new-${langKey}.png`);
  await clearMarks();

  // ── Admin tokens (tokens-and-api) ───────────────────────────────────
  await goto('/admin-tokens', byText('button', L.createToken));
  await clearMarks();
  await mark(byText('button', L.createToken), '1');
  await sleep(150);
  await shot(`admin-tokens-${langKey}.png`);
  await clearMarks();

  // ── Docs list (getting-started) ─────────────────────────────────────
  // Client-side navigation: a full page load of /docs races the language
  // restore against the doc-list resource during hydration and crashes the
  // WASM app in the non-default language. In-app routing avoids that.
  await evalJS(`(()=>{history.pushState({},'','/docs');dispatchEvent(new PopStateEvent('popstate'));})()`);
  await waitFor(byText('h2', L.docsTitle));
  await sleep(500);
  await evalJS('window.scrollTo(0,0)');
  await shot(`docs-list-${langKey}.png`);
}

try {
  for (const lang of ['en', 'de']) await capture(lang);
  console.log('done');
} finally {
  await send('Target.closeTarget', { targetId }).catch(() => {});
  ws.close();
  chrome.kill('SIGTERM');
  await sleep(500);
  try { rmSync(profile, { recursive: true, force: true }); } catch { /* chrome still releasing files */ }
}
