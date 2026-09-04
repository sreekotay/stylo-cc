// Shared formatting of StyleBench's benchmarkClient._measuredValuesList (one entry per iteration).
// ---------- formatting ----------
const fmt = (x, w = 9) => (typeof x === 'number' ? x.toFixed(1) : String(x)).padStart(w);
const STEP_KINDS = ['Adding classes', 'Removing classes', 'Mutating attributes', 'Adding leaf elements', 'Removing leaf elements', 'Resizing to'];

export function report(measuredValuesList, label) {
  console.log(`\n== ${label}: StyleBench, ${measuredValuesList.length} iteration(s), ms (sync = style + layout flush; async = constant 1/step) ==`);
  const suites = Object.keys(measuredValuesList[0].tests);
  console.log('\n' + 'suite'.padEnd(36) + 'iter'.padStart(5) + 'steps'.padStart(6) + 'sync ms'.padStart(10) + 'async ms'.padStart(10) + 'total ms'.padStart(10));
  const agg = {}; // suite -> { sync: [], total: [], steps, kinds: {kind: [sum per iter]} }
  measuredValuesList.forEach((mv, it) => {
    for (const s of suites) {
      const suite = mv.tests[s];
      const tests = Object.values(suite.tests);
      const sync = tests.reduce((a, t) => a + t.tests.Sync, 0);
      const asyn = tests.reduce((a, t) => a + t.tests.Async, 0);
      const a = (agg[s] ||= { sync: [], total: [], steps: tests.length, kinds: {} });
      a.sync.push(sync); a.total.push(suite.total);
      for (const [name, t] of Object.entries(suite.tests)) {
        const kind = STEP_KINDS.find(k => name.startsWith(k)) || name;
        (a.kinds[kind] ||= []); a.kinds[kind][it] = (a.kinds[kind][it] || 0) + t.tests.Sync;
      }
      console.log(s.padEnd(36) + String(it + 1).padStart(5) + String(tests.length).padStart(6) + fmt(sync, 10) + fmt(asyn, 10) + fmt(suite.total, 10));
    }
  });
  const mean = xs => xs.reduce((a, b) => a + b, 0) / xs.length;
  const min = xs => Math.min(...xs);
  console.log('\n' + 'suite (over iterations)'.padEnd(36) + 'steps'.padStart(6) + 'sync mean'.padStart(11) + 'sync min'.padStart(10) + 'total mean'.padStart(12));
  for (const s of suites) {
    const a = agg[s];
    console.log(s.padEnd(36) + String(a.steps).padStart(6) + fmt(mean(a.sync), 11) + fmt(min(a.sync), 10) + fmt(mean(a.total), 12));
  }
  console.log('\nper step kind, sync ms summed over the suite\'s steps of that kind, mean over iterations:');
  for (const s of suites) {
    const a = agg[s];
    const parts = Object.entries(a.kinds).map(([k, v]) => `${k.replace('Resizing to', 'resizes')}=${mean(v).toFixed(1)}`);
    console.log('  ' + s.padEnd(34) + parts.join('  '));
  }
  const scores = measuredValuesList.map(mv => mv.score);
  const geomeans = measuredValuesList.map(mv => mv.geomean);
  console.log(`\nStyleBench score (runs/min = 60000 / geomean(suite totals) / 1.5): mean ${mean(scores).toFixed(2)}  [${scores.map(x => x.toFixed(1)).join(', ')}]`);
  console.log(`geomean of suite totals, ms: mean ${mean(geomeans).toFixed(1)}  [${geomeans.map(x => x.toFixed(1)).join(', ')}]`);
}


export { fmt };
