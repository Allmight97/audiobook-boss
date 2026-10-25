// TEMPORARY: Playwright WebKit timings for the harness. Removed before merge.
// Usage: NODE_PATH=<dir with playwright> node perf/measure-webkit.mjs <label> [outFile]
import { createServer } from 'node:http';
import { appendFileSync, readFileSync, existsSync } from 'node:fs';
import { extname, join, resolve } from 'node:path';
import { createRequire } from 'node:module';

const require = createRequire(join(process.env.NODE_PATH ?? process.cwd(), 'noop.js'));
const { webkit } = require('playwright');
const label = process.argv[2] ?? 'unlabelled';
const out = process.argv[3];
const root = resolve(import.meta.dirname, 'dist');
const types = {
	'.html': 'text/html',
	'.js': 'text/javascript',
	'.css': 'text/css',
	'.svg': 'image/svg+xml',
};

const server = createServer((req, res) => {
	const path = join(root, decodeURIComponent(new URL(req.url, 'http://x').pathname));
	if (!existsSync(path)) return res.writeHead(404).end();
	res.writeHead(200, { 'content-type': types[extname(path)] ?? 'application/octet-stream' });
	res.end(readFileSync(path));
});
await new Promise((done) => server.listen(0, '127.0.0.1', done));
const base = `http://127.0.0.1:${server.address().port}/perf/index.html`;

const record = (row) => {
	const line = JSON.stringify({ label, browser: `playwright-webkit ${browser.version()}`, ...row });
	console.log(line);
	if (out) appendFileSync(out, `${line}\n`);
};
const pct = (values, p) =>
	[...values].sort((a, b) => a - b)[Math.min(values.length - 1, Math.floor(values.length * p))];
const round = (n) => +n.toFixed(1);

const browser = await webkit.launch();
async function open(scenario, titles, options = {}) {
	const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, ...options });
	const page = await context.newPage();
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	await page.goto(`${base}?scenario=${scenario}&titles=${titles}`);
	await page.waitForFunction(() => window.__abbPerf?.ready === true, null, { timeout: 120_000 });
	await page.evaluate(
		() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))),
	);
	return { page, context, errors };
}

// Running animations and transitions, by animated property. Deterministic idle-motion proof.
async function animations(page) {
	return page.evaluate(() =>
		document.getAnimations().map((animation) => ({
			type: animation.constructor.name,
			name: animation.animationName ?? animation.transitionProperty ?? '',
			state: animation.playState,
			iterations: animation.effect?.getTiming().iterations ?? null,
			target: animation.effect?.target?.className?.toString?.() ?? '',
		})),
	);
}

for (const scenario of ['idle', 'idle-complete']) {
	for (const reducedMotion of ['no-preference', 'reduce']) {
		const { page, context, errors } = await open(scenario, 0, { reducedMotion });
		await page.waitForTimeout(500);
		const running = (await animations(page)).filter((a) => a.state === 'running');
		record({
			metric: 'idle-animations',
			scenario,
			reducedMotion,
			running: running.length,
			detail: running,
			errors,
		});
		await context.close();
	}
}

for (const titles of [1000, 5000]) {
	{
		const { page, context, errors } = await open('download', titles);
		const ticks = await page.evaluate(async () => {
			const frame = () => new Promise((r) => requestAnimationFrame(() => r(performance.now())));
			const rows = [];
			for (let i = 1; i <= 12; i += 1) {
				await frame();
				const start = performance.now();
				const script = window.__abbPerf.tick(i * 8);
				await frame();
				const painted = await frame();
				rows.push({ script, toSecondFrame: painted - start });
			}
			return rows.slice(2);
		});
		const transitions = (await animations(page)).filter((a) => a.type === 'CSSTransition');
		record({
			metric: 'download-tick',
			titles,
			scriptMedianMs: round(
				pct(
					ticks.map((t) => t.script),
					0.5,
				),
			),
			scriptP95Ms: round(
				pct(
					ticks.map((t) => t.script),
					0.95,
				),
			),
			frameMedianMs: round(
				pct(
					ticks.map((t) => t.toSecondFrame),
					0.5,
				),
			),
			frameP95Ms: round(
				pct(
					ticks.map((t) => t.toSecondFrame),
					0.95,
				),
			),
			progressTransitions: transitions.map((t) => `${t.name}@${t.target}`),
			errors,
		});

		const typing = await page.evaluate(async () => {
			const frame = () => new Promise((r) => requestAnimationFrame(() => r(performance.now())));
			const input = document.getElementById('remote-source-filter');
			if (!input) return { error: 'no filter input' };
			const samples = [];
			const counts = [];
			for (const text of ['D', 'Du', 'Dun', 'Dune', 'Dun', 'Du', 'D', '', 'zzzz', '']) {
				await frame();
				const start = performance.now();
				input.value = text;
				input.dispatchEvent(new Event('input', { bubbles: true }));
				await frame();
				samples.push((await frame()) - start);
				counts.push(window.__abbPerf.rows());
			}
			return { samples, counts };
		});
		record({
			metric: 'filter-keystroke',
			titles,
			medianMs: typing.samples ? round(pct(typing.samples, 0.5)) : null,
			maxMs: typing.samples ? round(Math.max(...typing.samples)) : null,
			rowCounts: typing.counts,
			error: typing.error,
		});

		const scroll = await page.evaluate(async () => {
			const list = document.querySelector('.remote-title-list');
			if (!list) return { error: 'no list' };
			const scroller = (() => {
				for (let el = list; el; el = el.parentElement) {
					if (
						el.scrollHeight > el.clientHeight + 10 &&
						getComputedStyle(el).overflowY !== 'visible'
					)
						return el;
				}
				return document.scrollingElement;
			})();
			const intervals = [];
			let last = await new Promise((r) => requestAnimationFrame(r));
			for (let i = 0; i < 120; i += 1) {
				scroller.scrollTop += 240;
				const now = await new Promise((r) => requestAnimationFrame(r));
				intervals.push(now - last);
				last = now;
			}
			return {
				intervals,
				reachedBottom: scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 2,
			};
		});
		record({
			metric: 'scroll',
			titles,
			frameMedianMs: scroll.intervals ? round(pct(scroll.intervals, 0.5)) : null,
			frameP95Ms: scroll.intervals ? round(pct(scroll.intervals, 0.95)) : null,
			framesOver25Ms: scroll.intervals ? scroll.intervals.filter((v) => v > 25).length : null,
			frames: scroll.intervals?.length,
			error: scroll.error,
		});
		await context.close();
	}
	{
		const { page, context } = await open('download-closed', titles);
		const ticks = await page.evaluate(async () => {
			const rows = [];
			for (let i = 1; i <= 8; i += 1) rows.push(window.__abbPerf.tick(i * 10));
			return { rows, mountedRows: window.__abbPerf.rows() };
		});
		record({
			metric: 'download-tick-dialog-closed',
			titles,
			scriptMedianMs: round(pct(ticks.rows, 0.5)),
			mountedRows: ticks.mountedRows,
		});
		await context.close();
	}
}

await browser.close();
server.close();
