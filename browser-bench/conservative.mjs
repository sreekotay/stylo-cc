// "Conservative" StyleBench: WebKit's stock BenchmarkRunner._runTest flushes with
// body.getBoundingClientRect() only, which forces layout, and layout forces style -- unless the
// engine can tell the pending changes cannot affect geometry (e.g. a class flip whose rules only
// set color) and skips the style flush. Ladybird's perf harness (LadybirdBrowser/web-benchmarks,
// benchmarks/StyleBenchConservative/resources/conservative-runner.js) closes that hole by also
// reading getComputedStyle(body).backgroundColor before and after each step. This is the same
// override, verbatim in behaviour, injected as JS text after benchmark-runner.js has loaded.
// Results are still stock-StyleBench-shaped (sync per step, async = 1), so report.mjs is unchanged.
export const CONSERVATIVE_RUNTEST = `
window.styleBenchName = "StyleBenchConservative";
BenchmarkRunner.prototype._runTest = function (suite, test, prepareReturnValue, callback) {
    var self = this;
    var now = window.performance && window.performance.now ? function () { return window.performance.now(); } : Date.now;
    var contentWindow = self._frame.contentWindow;
    var contentDocument = self._frame.contentDocument;
    window._unusedBackgroundColorValue = contentWindow.getComputedStyle(contentDocument.body).backgroundColor;
    window._unusedHeightValue = contentDocument.body.getBoundingClientRect().height;
    self._writeMark(suite.name + '.' + test.name + '-start');
    var startTime = now();
    test.run(prepareReturnValue, contentWindow, contentDocument);
    window._unusedBackgroundColorValue = contentWindow.getComputedStyle(contentDocument.body).backgroundColor;
    window._unusedHeightValue = contentDocument.body.getBoundingClientRect().height;
    var endTime = now();
    self._writeMark(suite.name + '.' + test.name + '-sync-end');
    var syncTime = endTime - startTime;
    setTimeout(function () {
        var asyncTime = 1;
        self._writeMark(suite.name + '.' + test.name + '-async-end');
        callback(syncTime, asyncTime);
    }, 0);
};
`;
