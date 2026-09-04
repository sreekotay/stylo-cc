// Drive WebKit's StyleBench (vendored under browser-bench/StyleBench/) in a real
// browser via Playwright, and print per-suite timings.
//
//   node run.mjs --browser chrome|chromium|webkit [--iterations N] [--suite NAME]
//                [--headed] [--port P] [--json OUT] [--split]   (--first-restyle is an alias of --split)
//   node run.mjs --serve [--port P]          # just serve, for a manual Safari run
//   node run.mjs --from-json FILE            # format a pasted benchmarkClient._measuredValuesList
//
// What StyleBench measures (resources/benchmark-runner.js `_runTest`):
//   body.getBoundingClientRect()   // flush style+layout, outside the clock
//   start = performance.now()
//   step()                         // e.g. addClasses(100) — DOM edits only
//   body.getBoundingClientRect()   // forces style recalc AND layout
//   sync = now() - start;  async = 1 (a hard-coded constant, not measured)
// A suite total is sum(sync + 1) over its 25 steps (55 for the resize suite).
// Building the sheet + 20k-element tree, and the first style resolution, happen
// in the suite's `prepare` and are NOT timed by StyleBench. `--split` adds our own
// (non-StyleBench) probe that times that initial resolution and splits every flush
// into style-only vs layout.

import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));

// ---------- args ----------
const args = process.argv.slice(2);
const opt = (name, dflt) => {
  const i = args.indexOf('--' + name);
  if (i < 0) return dflt;
  const v = args[i + 1];
  return v === undefined || v.startsWith('--') ? true : v;
};
const browserName = opt('browser', null);
const iterations = parseInt(opt('iterations', '5'), 10);
const suiteFilter = opt('suite', null);
const headed = !!opt('headed', false);
const port = parseInt(opt('port', '8765'), 10);
const jsonOut = opt('json', null);
const serveOnly = !!opt('serve', false);
const fromJson = opt('from-json', null);
const firstRestyle = !!opt('first-restyle', false) || !!opt('split', false);

// ---------- static server ----------
const MIME = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.png': 'image/png', '.json': 'application/json' };
function startServer(port) {
  return new Promise((resolve, reject) => {
    const server = http.createServer((req, res) => {
      const urlPath = decodeURIComponent(new URL(req.url, 'http://x').pathname);
      let file = path.normalize(path.join(here, urlPath));
      if (!file.startsWith(here)) { res.writeHead(403); return res.end(); }
      if (fs.existsSync(file) && fs.statSync(file).isDirectory()) file = path.join(file, 'index.html');
      fs.readFile(file, (err, data) => {
        if (err) { res.writeHead(404); return res.end('not found'); }
        res.writeHead(200, { 'Content-Type': MIME[path.extname(file)] || 'application/octet-stream', 'Cache-Control': 'no-store' });
        res.end(data);
      });
    });
    server.on('error', reject);
    server.listen(port, '127.0.0.1', () => resolve(server));
  });
}

import { report, fmt } from './report.mjs';

// ---------- our own style-vs-layout probe (NOT part of StyleBench) ----------
// Same StyleBench class, same seeds, same step order as tests.js, but each flush is split into
// style-only, then layout:
//   style flush, Chromium: getComputedStyle(#testroot).color  -> Blink UpdateStyleAndLayoutTreeForElement
//                          (color is not layout-dependent -> whole-document style recalc, no layout)
//   style flush, WebKit:   document.getAnimations()            -> Document::updateStyleIfNeeded() -> resolveStyle()
//                          (WebKit's getComputedStyle only resolves the ancestor chain, and Blink's
//                          getAnimations() inside an iframe also lays out, so each engine gets its own primitive)
//   layout flush, both:    body.getBoundingClientRect()
// Reported per suite: initial style / initial layout (the first resolution after building the tree, which
// StyleBench never times), and the 25 (55) steps' style-only sum and layout sum. Steps run back-to-back in one
// task (StyleBench puts a setTimeout(0) between steps). Runs inside an 800x600 iframe like StyleBench so the
// resize suite's window.frameElement.style.width works. Best (lowest steps style) of 3 loads.
async function probeSplit(page, base, label, engine) {
  await page.goto(`${base}/StyleBench/index.html`);
  const names = await page.evaluate(() => StyleBench.predefinedConfigurations().map(c => c.name));
  console.log(`\n== ${label}: style / layout split probe (ours, not StyleBench; ms; best of 3 loads) ==`);
  console.log('suite'.padEnd(36) + 'build'.padStart(8) + 'init style'.padStart(11) + 'init layout'.padStart(12) + 'steps style'.padStart(12) + 'steps layout'.padStart(13) + 'steps sum'.padStart(10) + '  live');
  for (let i = 0; i < names.length; i++) {
    if (suiteFilter && names[i].toLowerCase() !== suiteFilter.toLowerCase()) continue;
    let best = null;
    for (let rep = 0; rep < 3; rep++) {
      await page.goto(`${base}/StyleBench/index.html`);
      const r = await page.evaluate(async ([idx, engine]) => {
        const cfg = StyleBench.predefinedConfigurations()[idx];
        const frame = document.createElement('iframe');
        Object.assign(frame.style, { width: '800px', height: '600px', border: '0', position: 'absolute', left: '0', top: '0' });
        frame.setAttribute('scrolling', 'no');
        const loaded = new Promise(r => frame.onload = r);
        frame.src = 'resources/style-bench.html';
        document.body.insertBefore(frame, document.body.firstChild);
        await loaded;
        const w = frame.contentWindow, d = frame.contentDocument;
        const root = d.getElementById('testroot');
        const flushStyle = engine === 'webkit' ? () => d.getAnimations() : () => w.getComputedStyle(root).color;
        const flushLayout = () => d.body.getBoundingClientRect().height;
        flushLayout();
        const t0 = performance.now();
        const bench = w.createBenchmark(cfg);
        const t1 = performance.now();
        flushStyle();
        const t2 = performance.now();
        flushLayout();
        const t3 = performance.now();
        let style = 0, layout = 0, steps = 0;
        const step = (fn) => { flushLayout(); const a = performance.now(); fn(); flushStyle(); const b = performance.now(); flushLayout(); const c = performance.now(); style += b - a; layout += c - b; steps++; };
        for (let s = 0; s < cfg.stepCount; s++) {
          if (cfg.isResizeTest) { for (let width = 300; width <= 800; width += 50) step(() => bench.resizeViewToWidth(width)); continue; }
          step(() => bench.addClasses(cfg.mutationsPerStep));
          step(() => bench.removeClasses(cfg.mutationsPerStep));
          step(() => bench.mutateAttributes(cfg.mutationsPerStep));
          step(() => bench.addLeafElements(cfg.mutationsPerStep));
          step(() => bench.removeLeafElements(cfg.mutationsPerStep));
        }
        const live = root.querySelectorAll('*').length;
        frame.remove();
        return { build: t1 - t0, initStyle: t2 - t1, initLayout: t3 - t2, style, layout, steps, live };
      }, [i, engine]);
      if (!best || r.style < best.style) best = r;
    }
    console.log(names[i].padEnd(36) + fmt(best.build, 8) + fmt(best.initStyle, 11) + fmt(best.initLayout, 12) + fmt(best.style, 12) + fmt(best.layout, 13) + fmt(best.style + best.layout, 10) + `  ${best.live} (${best.steps} steps)`);
  }
}

// ---------- main ----------
if (fromJson) {
  const j = JSON.parse(fs.readFileSync(fromJson, 'utf8'));
  const list = Array.isArray(j) ? j : j.measuredValuesList ? j.measuredValuesList : [j];
  report(list, j.label ? `${j.label} (from ${fromJson})` : `from ${fromJson}`);
  process.exit(0);
}

const server = await startServer(port);
const base = `http://127.0.0.1:${port}`;
// (StyleBench's own `suite=` query param does not URL-decode, and every suite name has spaces,
//  so we filter by setting Suites[i].disabled in-page instead.)
const url = `${base}/StyleBench/index.html?unit=ms&iterationCount=${iterations}`;

if (serveOnly || !browserName) {
  console.log(`Serving ${here} at ${base}`);
  console.log(`Open in Safari:  ${url}`);
  console.log(`then click "Start Test". When the Detailed Results page shows, in Web Inspector's console run:`);
  console.log(`  copy(JSON.stringify(benchmarkClient._measuredValuesList))`);
  console.log(`paste into a file and run:  node browser-bench/run.mjs --from-json FILE`);
  console.log(`Ctrl-C to stop the server.`);
  await new Promise(() => {});
}

const pw = await import('playwright');
let browser, label;
if (browserName === 'chrome') {
  browser = await pw.chromium.launch({ channel: 'chrome', headless: !headed });
  label = `Google Chrome ${browser.version()} (installed, ${headed ? 'headed' : 'headless=new'})`;
} else if (browserName === 'chromium') {
  browser = await pw.chromium.launch({ headless: !headed });
  label = `Playwright Chromium ${browser.version()} (${headed ? 'headed' : 'headless'})`;
} else if (browserName === 'webkit') {
  browser = await pw.webkit.launch({ headless: !headed });
  label = `Playwright WebKit ${browser.version()} (Playwright's own WebKit build, NOT Safari's shipped WebCore/JSC; ${headed ? 'headed' : 'headless'})`;
} else {
  console.error(`unknown --browser ${browserName}`);
  process.exit(2);
}

try {
  const context = await browser.newContext({ viewport: { width: 1024, height: 768 } });
  const page = await context.newPage();
  page.on('pageerror', e => console.error('page error:', e.message));
  page.on('dialog', async d => { console.error('dialog:', d.message()); await d.dismiss(); });

  if (firstRestyle) await probeSplit(page, base, label, browserName === 'webkit' ? 'webkit' : 'chromium');

  await page.goto(url);
  console.log(`\n${label}\nrunning ${url}`);
  const t0 = Date.now();
  if (suiteFilter) {
    const found = await page.evaluate((f) => {
      let found = false;
      for (const s of Suites) { s.disabled = s.name.toLowerCase() !== f.toLowerCase(); found ||= !s.disabled; }
      return found;
    }, suiteFilter);
    if (!found) throw new Error(`no suite named "${suiteFilter}"`);
  }
  await page.evaluate(() => startTest());
  // benchmarkClient._measuredValuesList gets one entry per finished iteration (didRunSuites).
  let done = 0;
  while (true) {
    const n = await page.evaluate(() => benchmarkClient._measuredValuesList.length);
    if (n > done) { done = n; process.stderr.write(`  iteration ${n}/${iterations} done (${((Date.now() - t0) / 1000).toFixed(0)}s)\n`); }
    if (n >= iterations) break;
    await page.waitForTimeout(500);
  }
  await page.waitForFunction(() => document.getElementById('results-with-statistics').textContent.trim() !== '');
  const list = await page.evaluate(() => benchmarkClient._measuredValuesList);
  const ua = await page.evaluate(() => navigator.userAgent);
  console.log(`userAgent: ${ua}`);
  report(list, label);
  if (jsonOut) { fs.writeFileSync(jsonOut, JSON.stringify({ label, ua, iterations, measuredValuesList: list }, null, 1)); console.log(`\nwrote ${jsonOut}`); }
} finally {
  await browser.close();
  server.close();
}
