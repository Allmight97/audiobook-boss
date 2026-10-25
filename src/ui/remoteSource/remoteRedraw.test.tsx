import { flush } from 'solid-js';
import { cleanup, render } from '@solidjs/testing-library';
import { afterEach, describe, expect, it } from 'vitest';
import { AppRuntimeProvider, createAppRuntime, type AppRuntime } from '../../app/runtime';
import { fakeEngine } from '../../test/fixtures/fakeEngine';
import type { AcquisitionJob, RemoteTitle } from '../../types/remoteSource';
import { RemoteSourceAcquireView } from './RemoteSourceAcquireView';

function library(count: number): RemoteTitle[] {
	return Array.from({ length: count }, (_, index) => ({
		providerId: 'audible',
		titleId: `B${String(index).padStart(9, '0')}`,
		title: `Measured Title ${index}`,
		authors: [`Author ${index}`],
		narrators: [],
		durationSeconds: 3600,
		supplementalPdfAvailable: false,
		acquired: false,
		availability: { status: 'available', acquirable: true, label: 'Available' },
		unsupportedReasons: [],
	}));
}

function job(percentage: number): AcquisitionJob {
	return {
		settled: false,
		terminal: false,
		jobId: 'remote-job-1',
		providerId: 'audible',
		status: 'acquiring',
		progress: {
			stage: 'download',
			percentage,
			message: 'Downloading audiobook.',
			terminal: false,
			bytesDownloaded: percentage * 1_000_000,
			bytesTotal: 100_000_000,
		},
		materializedFiles: [],
		supplementalAssets: [],
		diagnostics: [],
	};
}

describe('remote title rows during a download', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		cleanup();
		runtime?.dispose();
		runtime = undefined;
		document.body.innerHTML = '';
	});

	it('does not rebuild title rows when only the download tick changes', async () => {
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
			state.remoteLibrary.titles = library(1000);
			state.remoteLibrary.diagnostics = [];
		});
		runtime = createAppRuntime();
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<RemoteSourceAcquireView />
			</AppRuntimeProvider>
		));
		await runtime.remoteSource.open({ lane: 'audible' });
		await vi.waitFor(() => {
			expect(document.querySelectorAll('.remote-title-row').length).toBe(1000);
		});
		flush();

		let joins = 0;
		for (const title of runtime.remoteSource.view().titles) {
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

		const shown = runtime.remoteSource.view();
		const current = fakeEngine().remote();
		fakeEngine().notify({
			revision: current.revision + 1,
			remote: {
				...current,
				revision: current.revision + 1,
				acquisition: {
					...job(11),
					progress: { ...job(11).progress, percentage: 11 },
				},
			},
		});
		flush();

		expect(joins).toBe(0);
		expect(runtime.remoteSource.view()).toBe(shown);
		expect(runtime.remoteSource.progress()?.percentage).toBe(11);
		expect(document.querySelector('.remote-progress-copy')?.textContent).toContain('11%');
	});
});
