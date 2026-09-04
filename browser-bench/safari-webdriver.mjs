// Drive real Safari through safaridriver's WebDriver HTTP API (no selenium dependency).
// Requires Safari's Develop > Developer Settings > "Allow Remote Automation" to be on
// (scripts/browser-bench.sh safari checks that first). Safari has no headless mode; a window opens.
//
//   node safari-webdriver.mjs [--iterations N] [--suite NAME] [--port P] [--json OUT]

import { spawn } from 'node:child_process';
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { report } from './report.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const opt = (name, dflt) => { const i = args.indexOf('--' + name); return i < 0 ? dflt : (args[i + 1] ?? true); };
const iterations = parseInt(opt('iterations', '5'), 10);
const suiteFilter = opt('suite', null);
const port = parseInt(opt('port', '8765'), 10);
const jsonOut = opt('json', null);
const sdPort = 4446;

// static server (same as run.mjs)
const MIME = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.png': 'image/png' };
const server = http.createServer((req, res) => {
  let file = path.normalize(path.join(here, decodeURIComponent(new URL(req.url, 'http://x').pathname)));
  if (!file.startsWith(here)) { res.writeHead(403); return res.end(); }
  if (fs.existsSync(file) && fs.statSync(file).isDirectory()) file = path.join(file, 'index.html');
  fs.readFile(file, (err, data) => {
    if (err) { res.writeHead(404); return res.end(); }
    res.writeHead(200, { 'Content-Type': MIME[path.extname(file)] || 'application/octet-stream', 'Cache-Control': 'no-store' });
    res.end(data);
  });
});
await new Promise(r => server.listen(port, '127.0.0.1', r));
const url = `http://127.0.0.1:${port}/StyleBench/index.html?unit=ms&iterationCount=${iterations}`;

const sd = spawn('safaridriver', ['-p', String(sdPort)], { stdio: 'ignore' });
await new Promise(r => setTimeout(r, 1500));
const wd = async (method, p, body) => {
  const res = await fetch(`http://127.0.0.1:${sdPort}${p}`, { method, headers: { 'Content-Type': 'application/json' }, body: body ? JSON.stringify(body) : undefined });
  const j = await res.json();
  if (j.value && j.value.error) throw new Error(`${j.value.error}: ${j.value.message}`);
  return j.value;
};
let sid;
try {
  const s = await wd('POST', '/session', { capabilities: { alwaysMatch: { browserName: 'safari' } } });
  sid = s.sessionId;
  const ua = s.capabilities?.browserVersion;
  const exec = (script, ...a) => wd('POST', `/session/${sid}/execute/sync`, { script, args: a });
  await wd('POST', `/session/${sid}/window/rect`, { width: 1100, height: 800 });
  await wd('POST', `/session/${sid}/url`, { url });
  await new Promise(r => setTimeout(r, 1000));
  if (suiteFilter) {
    const found = await exec('const f=arguments[0].toLowerCase(); let found=false; for (const s of Suites){ s.disabled = s.name.toLowerCase()!==f; found ||= !s.disabled;} return found;', suiteFilter);
    if (!found) throw new Error(`no suite named "${suiteFilter}"`);
  }
  await exec('startTest()');
  const t0 = Date.now();
  let done = 0;
  while (true) {
    const n = await exec('return benchmarkClient._measuredValuesList.length');
    if (n > done) { done = n; process.stderr.write(`  iteration ${n}/${iterations} done (${((Date.now() - t0) / 1000).toFixed(0)}s)\n`); }
    if (n >= iterations) break;
    await new Promise(r => setTimeout(r, 500));
  }
  const list = await exec('return benchmarkClient._measuredValuesList');
  const label = `Safari ${ua ?? ''} (real Safari via safaridriver, headed)`;
  console.log(`userAgent: ${await exec('return navigator.userAgent')}`);
  report(list, label);
  if (jsonOut) fs.writeFileSync(jsonOut, JSON.stringify({ label, iterations, measuredValuesList: list }, null, 1));
} finally {
  if (sid) await wd('DELETE', `/session/${sid}`).catch(() => {});
  sd.kill();
  server.close();
}
