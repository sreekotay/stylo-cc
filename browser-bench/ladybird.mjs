// Drive WebKit's StyleBench (vendored under browser-bench/StyleBench/) in Ladybird
// (built from the ../ladybird submodule) and print per-suite timings.
//
//   node ladybird.mjs [--bin PATH] [--iterations N] [--suite NAME] [--json OUT] [--stall SEC] [--headed]
//                     [--conservative]   # StyleBenchConservative's _runTest (see conservative.mjs)
//                     [--internals]      # + Ladybird's own style clock per step (implies --conservative)
//
// Playwright cannot drive Ladybird, so this works the way Ladybird's own perf harness
// (LadybirdBrowser/web-benchmarks) does: serve StyleBench, but swap the stock
// resources/benchmark-report.js for a shim that auto-starts the run and POSTs progress and
// each iteration's measuredValues back to us; launch `Ladybird --headless=manual URL`; stop
// when /BenchmarkComplete arrives. The vendored StyleBench files are untouched: the shim is
// served in place of the stock file, not written over it. The measuredValues payload is the
// same object run.mjs reads out of benchmarkClient._measuredValuesList, so report.mjs is shared.
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { spawn, execSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { report } from './report.mjs';
import { CONSERVATIVE_RUNTEST } from './conservative.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const opt = (name, dflt) => {
  const i = args.indexOf('--' + name);
  if (i < 0) return dflt;
  const v = args[i + 1];
  return v === undefined || v.startsWith('--') ? true : v;
};
const bin = opt('bin', path.join(here, '..', 'ladybird', 'Build', 'distribution', 'bin', 'Ladybird.app', 'Contents', 'MacOS', 'Ladybird'));
const iterations = parseInt(opt('iterations', '5'), 10);
const suiteFilter = opt('suite', null);
const jsonOut = opt('json', null);
const stallSec = parseInt(opt('stall', '600'), 10); // Ladybird can take minutes per step on 20k elements
const headed = !!opt('headed', false);
const port = parseInt(opt('port', '8766'), 10);
const internals = !!opt('internals', false);
const conservative = !!opt('conservative', false) || internals;

if (!fs.existsSync(bin)) {
  console.error(`Ladybird binary not found at ${bin}\nbuild it:  cd ladybird && BUILD_PRESET=Distribution ./Meta/ladybird.py build ladybird`);
  process.exit(2);
}

// The report shim served as /StyleBench/resources/benchmark-report.js. Mirrors the hooks in
// LadybirdBrowser/web-benchmarks/benchmarks/StyleBench/resources/benchmark-report.js, plus our
// suite filter (StyleBench's own `suite=` param does not URL-decode and every name has spaces).
// --internals: Ladybird's own style clock. With --expose-internals-object every window (the
// StyleBench iframe included) gets `internals`, and internals.getStyleInvalidationCounters()
// returns Document::style_invalidation_counters: styleUpdateMicroseconds brackets the whole of
// Document::update_style (CSS/UpdateStyle.cpp; style only, never layout), with sub-timers for the
// Rust StyleEngine's transaction setup + planning (matching / invalidation) and the C++ recompute
// (materializing ComputedValues). This _runTest keeps the Conservative flush and samples those
// counters around (a) the pre-step flush -- which for a suite's first test is the initial
// resolution StyleBench never times -- and (b) the step itself. Deltas are POSTed per step.
const INTERNALS_RUNTEST = `
window.styleBenchName = "StyleBenchInternals";
var LB_KEYS = ["styleUpdateMicroseconds", "styleEngineTransactionSetupMicroseconds", "styleEnginePlanningMicroseconds", "styleRecomputeMicroseconds", "styleCascadeMicroseconds", "styleValuesMicroseconds", "mediaRuleEvaluations", "styleEngineTransactionSetups"];
var lbSample = function (w) { var c = w.internals.getStyleInvalidationCounters(); var o = {}; for (var k of LB_KEYS) o[k] = Number(c[k] || 0); return o; };
var lbDelta = function (a, b) { var o = {}; for (var k of LB_KEYS) o[k] = b[k] - a[k]; return o; };
var lbFlush = function (w, d) { window._unusedBackgroundColorValue = w.getComputedStyle(d.body).backgroundColor; window._unusedHeightValue = d.body.getBoundingClientRect().height; };
var lbPost = function (path, body) { var x = new XMLHttpRequest(); x.open("POST", path); x.setRequestHeader("Content-Type", "application/json"); x.send(JSON.stringify(body)); };
// Initial resolution: flush right after createBenchmark() returns, in the same task, before the
// setTimeout that precedes the first step gives Ladybird a rendering opportunity. (SimplePromise
// runs its callbacks synchronously, so this is still inside prepare's task.)
BenchmarkState.prototype.prepareCurrentSuite = function (runner, frame) {
    var suite = this.currentSuite();
    var promise = new SimplePromise;
    frame.onload = function () {
        suite.prepare(runner, frame.contentWindow, frame.contentDocument).then(function (result) {
            var w = frame.contentWindow, d = frame.contentDocument;
            var s0 = lbSample(w); var t0 = performance.now();
            lbFlush(w, d);
            var t1 = performance.now(); var s1 = lbSample(w);
            lbPost("/SuitePrepared", { suite: suite.name, wall: t1 - t0, init: lbDelta(s0, s1), live: d.getElementById('testroot').querySelectorAll('*').length });
            promise.resolve(result);
        });
    };
    frame.src = 'resources/' + suite.url;
    return promise;
};
BenchmarkRunner.prototype._runTest = function (suite, test, prepareReturnValue, callback) {
    var self = this;
    var now = function () { return window.performance.now(); };
    var w = self._frame.contentWindow, d = self._frame.contentDocument;
    var s0 = lbSample(w);
    lbFlush(w, d);
    var s1 = lbSample(w);
    self._writeMark(suite.name + '.' + test.name + '-start');
    var startTime = now();
    test.run(prepareReturnValue, w, d);
    lbFlush(w, d);
    var endTime = now();
    var s2 = lbSample(w);
    self._writeMark(suite.name + '.' + test.name + '-sync-end');
    lbPost("/StepCounters", { suite: suite.name, test: test.name, sync: endTime - startTime, pre: lbDelta(s0, s1), step: lbDelta(s1, s2) });
    var syncTime = endTime - startTime;
    setTimeout(function () {
        var asyncTime = 1;
        self._writeMark(suite.name + '.' + test.name + '-async-end');
        callback(syncTime, asyncTime);
    }, 0);
};
`;

const SHIM = (internals ? INTERNALS_RUNTEST : conservative ? CONSERVATIVE_RUNTEST : '') + `
(function () {
    const post = (path, body) => { const x = new XMLHttpRequest(); x.open("POST", path); x.setRequestHeader("Content-Type", "application/json"); x.send(JSON.stringify(body)); };
    window.onerror = (m, s, l) => post("/PageError", { message: String(m), source: String(s), line: l });
    window.onload = function () {
        const only = ${JSON.stringify(suiteFilter)};
        if (only) for (const s of Suites) s.disabled = s.name.toLowerCase() !== only.toLowerCase();
        startBenchmark();
        showSection("running");
        const c = window.benchmarkClient;
        const wrap = (name, fn) => { const o = c[name]; c[name] = function (...a) { o.apply(this, a); fn(...a); }; };
        wrap("willRunTest", (suite, test) => post("/TestStarting", { suite: suite.name, test: test.name }));
        wrap("didRunTest", (suite, test) => post("/TestComplete", { suite: suite.name, test: test.name }));
        wrap("didRunSuites", (mv) => post("/IterationComplete", { results: mv }));
        wrap("didFinishLastIteration", () => post("/BenchmarkComplete", { ua: navigator.userAgent }));
    };
})();
`;

const MIME = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.png': 'image/png', '.json': 'application/json' };
const list = [];
const steps = []; // --internals: one entry per step per iteration
const prepared = []; // --internals: one entry per suite per iteration (initial resolution)
let ua = '';
let lastProgress = Date.now();
let finished = false;
const t0 = Date.now();
const elapsed = () => ((Date.now() - t0) / 1000).toFixed(0) + 's';

const server = http.createServer((req, res) => {
  const urlPath = decodeURIComponent(new URL(req.url, 'http://x').pathname);
  if (req.method === 'POST') {
    let body = '';
    req.on('data', d => body += d);
    req.on('end', () => {
      lastProgress = Date.now();
      let j = {}; try { j = JSON.parse(body || '{}'); } catch {}
      if (urlPath === '/TestStarting') process.stderr.write(`  [${elapsed()}] ${j.suite} / ${j.test}\n`);
      else if (urlPath === '/IterationComplete') { list.push(j.results); process.stderr.write(`  [${elapsed()}] iteration ${list.length}/${iterations} done\n`); }
      else if (urlPath === '/BenchmarkComplete') { ua = j.ua || ''; finished = true; }
      else if (urlPath === '/StepCounters') steps.push({ iter: list.length, ...j });
      else if (urlPath === '/SuitePrepared') prepared.push({ iter: list.length, ...j });
      else if (urlPath === '/PageError') console.error(`page error: ${j.message} (${j.source}:${j.line})`);
      res.writeHead(200); res.end();
    });
    return;
  }
  if (urlPath === '/StyleBench/resources/benchmark-report.js') {
    res.writeHead(200, { 'Content-Type': 'text/javascript', 'Cache-Control': 'no-store' });
    return res.end(SHIM);
  }
  let file = path.normalize(path.join(here, urlPath));
  if (!file.startsWith(here)) { res.writeHead(403); return res.end(); }
  if (fs.existsSync(file) && fs.statSync(file).isDirectory()) file = path.join(file, 'index.html');
  fs.readFile(file, (err, data) => {
    if (err) { res.writeHead(404); return res.end('not found'); }
    res.writeHead(200, { 'Content-Type': MIME[path.extname(file)] || 'application/octet-stream', 'Cache-Control': 'no-store' });
    res.end(data);
  });
});
await new Promise((r, j) => { server.on('error', j); server.listen(port, '127.0.0.1', r); });

const url = `http://127.0.0.1:${port}/StyleBench/index.html?unit=ms&iterationCount=${iterations}`;
const lbArgs = [...(internals ? ['--expose-internals-object'] : []), ...(headed ? [] : ['--headless=manual', '--window-width=1024', '--window-height=768']), url];
const label = `Ladybird ${gitDesc()} (Distribution build, ${headed ? 'headed' : 'headless=manual'}${internals ? ', CONSERVATIVE flush + internals style counters' : conservative ? ', CONSERVATIVE runner: +getComputedStyle flush' : ''})`;
console.log(`${label}\n${bin}\nrunning ${url}`);

const child = spawn(bin, lbArgs, { stdio: ['ignore', 'pipe', 'pipe'] });
const verbose = !!opt('verbose', false);
child.stdout.on('data', d => { if (verbose) process.stderr.write(d); });
child.stderr.on('data', d => { if (verbose) process.stderr.write(d); });
let exited = false;
child.on('exit', (code, sig) => { exited = true; if (!finished) console.error(`Ladybird exited early (code ${code}, signal ${sig})`); });

while (!finished && !exited) {
  if (Date.now() - lastProgress > stallSec * 1000) { console.error(`no progress for ${stallSec}s; giving up`); break; }
  await new Promise(r => setTimeout(r, 500));
}
if (!exited) { child.kill('SIGINT'); await new Promise(r => setTimeout(r, 2000)); if (!exited) child.kill('SIGKILL'); }
server.close();

if (!list.length) { console.error('no iterations completed'); process.exit(1); }
if (ua) console.log(`userAgent: ${ua}`);
report(list, label);
if (internals) reportInternals(steps, prepared);
if (jsonOut) { fs.writeFileSync(jsonOut, JSON.stringify({ label, ua, iterations, measuredValuesList: list, ...(internals ? { steps, prepared } : {}) }, null, 1)); console.log(`\nwrote ${jsonOut}`); }

// Per suite, per iteration: StyleBench's sync clock next to Ladybird's own update_style clock for the
// same steps, split into the Rust engine (transaction setup + planning) and the C++ recompute; plus the
// initial resolution (the flush we force right after createBenchmark). Then the min over iterations.
function reportInternals(steps, prepared) {
  const us = (o, k) => (o[k] || 0) / 1000;
  const suites = [...new Set(steps.map(s => s.suite))];
  const cols = ['iter', 'steps', 'sync', 'style', 'engine', 'recompute', 'pre-flush', 'init style', 'init engine', 'init wall', 'live'];
  const W = [5, 6, 9, 9, 9, 10, 10, 11, 12, 10, 7];
  const row = (name, vals) => name.padEnd(36) + vals.map((v, i) => (typeof v === 'number' ? (Number.isInteger(v) ? String(v) : v.toFixed(1)) : String(v)).padStart(W[i])).join('');
  console.log(`\n== ${label}: Ladybird internals style clock, ms (Document::update_style; style only, no layout) ==`);
  console.log(row('suite', cols));
  const per = {};
  for (const suite of suites) {
    const iters = [...new Set(steps.filter(s => s.suite === suite).map(s => s.iter))];
    for (const it of iters) {
      const ss = steps.filter(s => s.suite === suite && s.iter === it);
      const p = prepared.find(x => x.suite === suite && x.iter === it) || { wall: 0, init: {}, live: 0 };
      const r = {
        n: ss.length,
        sync: ss.reduce((a, s) => a + s.sync, 0),
        style: ss.reduce((a, s) => a + us(s.step, 'styleUpdateMicroseconds'), 0),
        engine: ss.reduce((a, s) => a + us(s.step, 'styleEngineTransactionSetupMicroseconds') + us(s.step, 'styleEnginePlanningMicroseconds'), 0),
        recompute: ss.reduce((a, s) => a + us(s.step, 'styleRecomputeMicroseconds'), 0),
        pre: ss.reduce((a, s) => a + us(s.pre, 'styleUpdateMicroseconds'), 0),
        init: us(p.init, 'styleUpdateMicroseconds'),
        initEngine: us(p.init, 'styleEngineTransactionSetupMicroseconds') + us(p.init, 'styleEnginePlanningMicroseconds'),
        initWall: p.wall,
        live: p.live,
      };
      (per[suite] ||= []).push(r);
      console.log(row(suite, [it + 1, r.n, r.sync, r.style, r.engine, r.recompute, r.pre, r.init, r.initEngine, r.initWall, r.live]));
    }
  }
  console.log('\n' + row('suite (min over iterations)', cols.map(c => c === 'iter' ? '' : c)));
  const min = (xs, k) => Math.min(...xs.map(x => x[k]));
  for (const suite of suites) {
    const xs = per[suite];
    console.log(row(suite, ['', xs[0].n, min(xs, 'sync'), min(xs, 'style'), min(xs, 'engine'), min(xs, 'recompute'), min(xs, 'pre'), min(xs, 'init'), min(xs, 'initEngine'), min(xs, 'initWall'), xs[0].live]));
  }
  console.log('\nsync = StyleBench step clock (JS + style + layout). style = styleUpdateMicroseconds over the suite\'s steps; engine = styleEngineTransactionSetup + styleEnginePlanning (Rust StyleEngine:');
  console.log('matching / invalidation / cascade identities); recompute = styleRecomputeMicroseconds (C++ ComputedValues materialization). pre-flush = update_style spent in the pre-step flushes (work');
  console.log('deferred out of a step lands here). init = the flush we force right after createBenchmark(): style / engine part / wall (incl. layout); live = elements under #testroot.');
}

function gitDesc() {
  try {
    return execSync('git -C ../ladybird log -1 --format=%h_%cs', { cwd: here }).toString().trim().replace('_', ' ');
  } catch { return ''; }
}
