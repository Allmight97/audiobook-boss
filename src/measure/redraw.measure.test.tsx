/**
 * Temporary before/after measurement for remote-library redraws.
 * Removed once the regression test covers the same tick.
 */
import { createEffect, createRoot, flush } from 'solid-js';
import { cleanup, fireEvent, render } from '@solidjs/testing-library';
import { afterEach, describe, expect, it } from 'vitest';
import { AppRuntimeProvider, createAppRuntime, type AppRuntime } from '../app/runtime';
import { fakeEngine } from '../test/fixtures/fakeEngine';
import type { AcquisitionJob, RemoteTitle } from '../types/remoteSource';
import { RemoteSourceAcquireView } from '../ui/remoteSource';

function titles(count: number): RemoteTitle[] {
	return Array.from({ length: count }, (_, index) => ({
		providerId: 'audible',
		titleId: `B${String(index).padStart(9, '0')}`,
		title: `Measured Title ${index}`,
		authors: [`Author ${index % 40}`],
		narrators: [],
		durationSeconds: 3600,
		supplementalPdfAvailable: index % 5 === 0,
		acquired: false,
		availability: { status: 'available', acquirable: true, label: 'Available' },
		unsupportedReasons: [],
	}));
}

/** Counts a row body execution: the row joins authors once per run. */
function countAuthorJoins(rows: RemoteTitle[]): void {
	for (const title of rows) {
		const authors = [...title.authors];
		title.authors = new Proxy(authors, {
			get(target, prop, receiver) {
				if (prop === 'join') {
					return (separator: string) => {
						joins += 1;
						return target.join(separator);
					};
				}
				return Reflect.get(target, prop, receiver);
			},
		});
	}
}

function job(percentage: number): AcquisitionJob {
	return {
		settled: false,
		terminal: false,
		jobId: 'remote-job-measure',
		providerId: 'audible',
		status: 'acquiring',
		progress: {
			stage: 'download',
			percentage,
			message: 'Downloading audiobook.',
			terminal: false,
			currentTitleId: 'B000000000',
			currentItemIndex: 1,
			totalItems: 1,
			bytesDownloaded: percentage * 1_000_000,
			bytesTotal: 100_000_000,
		},
		materializedFiles: [],
		supplementalAssets: [],
		diagnostics: [],
	};
}

let joins = 0;
let runtime: AppRuntime | undefined;
let stopWatch: (() => void) | undefined;

afterEach(() => {
	cleanup();
	stopWatch?.();
	stopWatch = undefined;
	runtime?.dispose();
	runtime = undefined;
	document.body.innerHTML = '';
});

async function mount(count: number): Promise<{ viewNotifications: () => number }> {
	joins = 0;
	fakeEngine().change((state) => {
		state.remote.lane = 'audible';
		state.remote.providers = [
			{ providerId: 'audible', label: 'Audible' },
			{ providerId: 'indexer', label: 'Indexer' },
		];
		state.remote.account = { providerId: 'audible', status: 'connected' };
		state.remote.libraryStatus = { kind: 'succeeded' };
		state.remote.acquiring = true;
		state.remote.acquisition = job(10);
		state.remoteLibrary.titles = titles(count);
		state.remoteLibrary.diagnostics = [];
	});
	runtime = createAppRuntime();
	let viewNotifications = 0;
	createRoot((dispose) => {
		stopWatch = dispose;
		createEffect(
			() => runtime?.remoteSource.view(),
			() => {
				viewNotifications += 1;
			},
		);
	});
	render(() => (
		<AppRuntimeProvider runtime={runtime!}>
			<RemoteSourceAcquireView />
		</AppRuntimeProvider>
	));
	await runtime.remoteSource.open({ lane: 'audible' });
	await vi.waitFor(() => {
		expect(document.querySelectorAll('.remote-title-row').length).toBe(count);
	});
	flush();
	countAuthorJoins(runtime.remoteSource.view().titles);
	joins = 0;
	return { viewNotifications: () => viewNotifications };
}

describe('remote redraw measurement', () => {
	it('records row work for one download tick and one filter keystroke', async () => {
		const samples: Record<string, unknown>[] = [];
		for (const count of [1000, 5000]) {
			cleanup();
			stopWatch?.();
			runtime?.dispose();
			const watch = await mount(count);
			const joinsBefore = joins;
			const viewsBefore = watch.viewNotifications();
			const current = fakeEngine().remote();
			const tickStart = performance.now();
			fakeEngine().notify({
				revision: current.revision + 1,
				remote: {
					...current,
					revision: current.revision + 1,
					acquisition: current.acquisition
						? {
								...current.acquisition,
								progress: {
									...current.acquisition.progress,
									percentage: current.acquisition.progress.percentage + 1,
								},
							}
						: null,
				},
			});
			flush();
			const tickMs = performance.now() - tickStart;
			const input = document.getElementById('remote-source-filter') as HTMLInputElement;
			const filterStart = performance.now();
			await fireEvent.input(input, { target: { value: 'Measured Title 42' } });
			flush();
			const filterMs = performance.now() - filterStart;
			const visible = document.querySelectorAll('.remote-title-row').length;
			samples.push({
				titles: count,
				rowJoinsOnTick: joins - joinsBefore,
				viewNotificationsOnTick: watch.viewNotifications() - viewsBefore,
				tickMs: Number(tickMs.toFixed(2)),
				filterMs: Number(filterMs.toFixed(2)),
				rowsAfterFilter: visible,
			});
		}
		const stamped = { at: new Date().toISOString(), samples };
		console.log(`redraw-measure ${JSON.stringify(stamped)}`);
		expect(samples).toHaveLength(2);
	}, 120_000);
});
