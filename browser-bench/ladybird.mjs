// Drive WebKit's StyleBench (vendored under browser-bench/StyleBench/) in Ladybird
// (built from the ../ladybird submodule) and print per-suite timings.
//
//   node ladybird.mjs [--bin PATH] [--iterations N] [--suite NAME] [--json OUT] [--stall SEC] [--headed]
//                     [--conservative]   # StyleBenchConservative's _runTest (see conservative.mjs)
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
const conservative = !!opt('conservative', false);

if (!fs.existsSync(bin)) {
  console.error(`Ladybird binary not found at ${bin}\nbuild it:  cd ladybird && BUILD_PRESET=Distribution ./Meta/ladybird.py build ladybird`);
  process.exit(2);
}

// The report shim served as /StyleBench/resources/benchmark-report.js. Mirrors the hooks in
// LadybirdBrowser/web-benchmarks/benchmarks/StyleBench/resources/benchmark-report.js, plus our
// suite filter (StyleBench's own `suite=` param does not URL-decode and every name has spaces).
const SHIM = (conservative ? CONSERVATIVE_RUNTEST : '') + `
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
const lbArgs = headed ? [url] : ['--headless=manual', '--window-width=1024', '--window-height=768', url];
const label = `Ladybird ${gitDesc()} (Distribution build, ${headed ? 'headed' : 'headless=manual'}${conservative ? ', CONSERVATIVE runner: +getComputedStyle flush' : ''})`;
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
if (jsonOut) { fs.writeFileSync(jsonOut, JSON.stringify({ label, ua, iterations, measuredValuesList: list }, null, 1)); console.log(`\nwrote ${jsonOut}`); }

function gitDesc() {
  try {
    return execSync('git -C ../ladybird log -1 --format=%h_%cs', { cwd: here }).toString().trim().replace('_', ' ');
  } catch { return ''; }
}
