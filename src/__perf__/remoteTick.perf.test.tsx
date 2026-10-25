// TEMPORARY measurement for the idle-redraw / remote-view PR. Run with ABB_PERF=1; removed before merge.
import { flush } from 'solid-js';
import { cleanup, render } from '@solidjs/testing-library';
import { afterEach, describe, expect, it } from 'vitest';
import { AppRuntimeProvider, createAppRuntime, type AppRuntime } from '../app/runtime';
import { createFakeEngine } from '../test/fixtures/fakeEngine';
import { RemoteSourceAcquireView } from '../ui/remoteSource';
import { progressTick, seedDownload, tickableEngine } from './fixture';

declare const process: { readonly env: Record<string, string | undefined> };
const enabled = process.env.ABB_PERF === '1';
type Fs = { appendFileSync(path: string, data: string): void };
const loadFs = async () => (await import(/* @vite-ignore */ ['node', 'fs'].join(':'))) as Fs;
const median = (values: number[]) =>
	[...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];

describe.runIf(enabled)('remote view cost per download tick (measurement)', () => {
	let runtime: AppRuntime | undefined;
	afterEach(() => {
		cleanup();
		runtime?.dispose();
		runtime = undefined;
	});

	for (const count of [1000, 5000]) {
		it(`${count} titles`, async () => {
			const engine = createFakeEngine();
			const tickable = tickableEngine(engine);
			const seeded = seedDownload(engine, count);
			runtime = createAppRuntime({ engine: tickable.capability });
			await runtime.initialize();
			await runtime.remoteSource.open({ lane: 'audible' });

			let viewReads = 0;
			const owner = runtime.remoteSource as { view: () => unknown };
			const original = owner.view;
			owner.view = () => {
				viewReads += 1;
				return original();
			};
			const app = runtime;
			render(() => (
				<AppRuntimeProvider runtime={app}>
					<RemoteSourceAcquireView />
				</AppRuntimeProvider>
			));
			flush();
			const rows = () => document.querySelectorAll('.remote-title-row');
			expect(rows().length).toBe(count);
			const firstRow = rows()[0];

			let mutations = 0;
			const list = document.querySelector('.remote-title-list') as Element;
			const observer = new MutationObserver((records) => {
				mutations += records.length;
			});
			observer.observe(list, {
				subtree: true,
				attributes: true,
				childList: true,
				characterData: true,
			});

			const samples: number[] = [];
			const reads: number[] = [];
			const received = tickable.received();
			for (let tick = 1; tick <= 7; tick += 1) {
				viewReads = 0;
				const update = progressTick(seeded, tick * 10);
				const start = performance.now();
				tickable.emit(update);
				flush();
				samples.push(performance.now() - start);
				reads.push(viewReads);
			}
			await Promise.resolve();
			observer.disconnect();
			expect(tickable.received()).toBeGreaterThan(received);
			expect(document.body.textContent).toContain('70');
			expect(rows()[0]).toBe(firstRow);
			(await loadFs()).appendFileSync(
				process.env.ABB_PERF_OUT ?? '/dev/stdout',
				`${JSON.stringify({
					metric: 'remote-tick-jsdom',
					label: process.env.ABB_PERF_LABEL ?? '',
					titles: count,
					medianMs: +median(samples).toFixed(1),
					maxMs: +Math.max(...samples).toFixed(1),
					viewReadsPerTick: median(reads),
					rowListMutations: mutations,
					ticks: samples.length,
				})}\n`,
			);
		}, 120_000);
	}
});
