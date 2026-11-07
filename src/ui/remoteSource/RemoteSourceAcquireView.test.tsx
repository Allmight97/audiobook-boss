import { flush } from 'solid-js';
import { createFakeEngine, fakeEngine, sessionUpdate } from '../../test/fixtures/fakeEngine';
import { cleanup, fireEvent, render, screen } from '@solidjs/testing-library';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AppRuntimeProvider, createAppRuntime, type AppRuntime } from '../../app/runtime';

import type { SessionUpdate } from '../../types/session';
import type {
	AcquisitionJob,
	RemoteSourceProviderCapabilities,
	RemoteTitle,
	RemoteRelease,
} from '../../types/remoteSource';
import { RemoteSourceAcquireView } from './RemoteSourceAcquireView';

function acquisitionJob(percentage: number, terminal = false): AcquisitionJob {
	return {
		settled: terminal,
		terminal,
		jobId: 'remote-job-1',
		providerId: 'audible',
		status: terminal ? 'validated' : 'acquiring',
		progress: {
			stage: terminal ? 'importHandoff' : 'download',
			percentage,
			message: terminal ? 'Acquisition complete.' : 'Downloading audiobook.',
			terminal,
			bytesDownloaded: null,
			bytesTotal: null,
			currentTitleId: 'B000000001',
			currentItemIndex: 1,
			totalItems: 1,
		},
		diagnostics: [],
	};
}

function providerCapabilities(): RemoteSourceProviderCapabilities[] {
	return [
		{
			providerId: 'audible',
			label: 'Audible',
		},
		{
			providerId: 'indexer',
			label: 'Indexer',
		},
	];
}

function remoteTitle(): RemoteTitle {
	return {
		providerId: 'audible',
		titleId: 'B000000001',
		title: 'Example Book',
		authors: ['Example Author'],
		narrators: [],
		cover: null,
		supplementalPdfAvailable: false,
		availability: {
			status: 'available',
			acquirable: true,
			label: 'Available',
			detail: null,
		},
	};
}

async function openConnected(
	runtime: AppRuntime,
	lane: 'audible' | 'indexer',
	releases?: RemoteRelease[],
) {
	fakeEngine().change((state) => {
		state.remote.lane = lane;
		state.remote.providers = providerCapabilities();
		state.remote.account = { providerId: lane, status: 'connected', message: null };
		state.remoteLibrary.titles = [remoteTitle()];
	});
	await runtime.remoteSource.open({ lane });
	if (releases) {
		fakeEngine().change((state) => {
			state.remote.indexer.releases = releases;
		});
		runtime.remoteSource.editSearch({ indexerTitleQuery: 'Example' });
		await runtime.remoteSource.runAction({ type: 'searchReleases' });
	}
}

describe('RemoteSourceAcquireView', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		cleanup();
		runtime?.dispose();
		runtime = undefined;
		vi.restoreAllMocks();
		document.body.innerHTML = '';
	});

	it('hides on Escape or Close and leaves a running acquisition running', async () => {
		runtime = await createAppRuntime();
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<RemoteSourceAcquireView />
			</AppRuntimeProvider>
		));
		const cancelSent = () =>
			fakeEngine().sessionIntents.some(
				(sent) => sent.kind === 'remote' && sent.intent.kind === 'cancelAcquisition',
			);
		await openConnected(runtime, 'audible');
		fakeEngine().change((state) => {
			state.remoteAcquisition.job = acquisitionJob(40);
			state.remote.busy = true;
		});
		flush();

		fireEvent.keyDown(document.body, { key: 'Escape' });
		flush();
		expect(runtime.remoteSource.view().isOpen).toBe(false);
		expect(cancelSent()).toBe(false);

		await runtime.remoteSource.open({ lane: 'audible' });
		flush();
		expect(runtime.remoteSource.view().isOpen).toBe(true);
		fireEvent.click(document.getElementById('remote-source-close')!);
		flush();
		expect(runtime.remoteSource.view().isOpen).toBe(false);
		expect(cancelSent()).toBe(false);
	});

	it('renders engine acquisition snapshots only in the Audible lane', async () => {
		runtime = await createAppRuntime();
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<RemoteSourceAcquireView />
			</AppRuntimeProvider>
		));
		await openConnected(runtime, 'audible');
		fakeEngine().change((state) => {
			state.remoteAcquisition.job = acquisitionJob(40);
			state.remote.selectedTitleIds = [remoteTitle().titleId];
			state.remote.busy = true;
		});
		flush();
		const bar = () => screen.getByRole('progressbar', { name: 'Acquisition progress' });
		expect(bar()).toHaveAttribute('aria-valuenow', '40');
		expect(bar()).toHaveAttribute('data-active');
		expect(screen.getByRole('option', { name: new RegExp(remoteTitle().title) })).toHaveAttribute(
			'aria-selected',
			'true',
		);
		expect(screen.getByRole('button', { name: 'Refresh Library' })).toBeDisabled();
		expect(screen.getByRole('button', { name: 'Cancel Acquisition' })).toBeEnabled();
		fireEvent.click(screen.getByRole('button', { name: 'Cancel Acquisition' }));
		flush();
		await vi.waitFor(() =>
			expect(fakeEngine().sessionIntents).toContainEqual({
				kind: 'remote',
				intent: { kind: 'cancelAcquisition', jobId: 'remote-job-1' },
			}),
		);
		fakeEngine().change((state) => {
			state.remoteAcquisition.job = acquisitionJob(100, true);
			state.remote.busy = false;
		});
		flush();
		const user = userEvent.setup();
		await user.selectOptions(screen.getByLabelText('Source'), 'indexer');
		fakeEngine().change((state) => {
			state.remote.lane = 'indexer';
		});
		flush();
		expect(screen.queryByRole('progressbar')).not.toBeInTheDocument();
		await user.selectOptions(screen.getByLabelText('Source'), 'audible');
		fakeEngine().change((state) => {
			state.remote.lane = 'audible';
		});
		flush();
		expect(bar()).toHaveAttribute('aria-valuenow', '100');
		expect(bar()).not.toHaveAttribute('data-active');
	});

	it('exposes Audible controls and filters connected library titles', async () => {
		runtime = await createAppRuntime();
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<RemoteSourceAcquireView />
			</AppRuntimeProvider>
		));
		await openConnected(runtime, 'audible');
		await Promise.resolve();

		expect(screen.getByLabelText('Source')).toBeEnabled();
		expect(screen.getByRole('button', { name: 'Refresh Library' })).toBeEnabled();
		expect(screen.getByRole('button', { name: 'Acquire Selected' })).toBeDisabled();
		expect(screen.getByRole('checkbox', { name: 'Supplemental PDF only' })).not.toBeChecked();
		expect(screen.getByRole('checkbox', { name: 'Hide unavailable' })).not.toBeChecked();
		expect(
			screen.getByRole('option', { name: new RegExp(remoteTitle().title) }),
		).toBeInTheDocument();
		fireEvent.input(screen.getByLabelText('Filter'), {
			target: { value: 'no matching book' },
		});
		flush();
		expect(screen.queryByText(remoteTitle().title)).not.toBeInTheDocument();
	});

	it('switches source lanes from the enabled provider control', async () => {
		runtime = await createAppRuntime();
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<RemoteSourceAcquireView />
			</AppRuntimeProvider>
		));
		await openConnected(runtime, 'audible');
		fakeEngine().respond = (intent) => {
			if (intent.kind === 'remote' && intent.intent.kind === 'selectLane') {
				fakeEngine().change((state) => {
					state.remote.lane = 'indexer';
					state.remote.account = {
						providerId: 'indexer',
						status: 'needsAuth',
						message: 'Configure Indexer URL and API key in Settings before searching.',
					};
				});
			}
			return undefined;
		};
		await Promise.resolve();

		const user = userEvent.setup();
		await user.selectOptions(screen.getByTestId('remote-source-provider'), 'indexer');
		await Promise.resolve();

		await vi.waitFor(() => expect(runtime!.remoteSource.view().providerId).toBe('indexer'), {
			timeout: 2000,
		});
		expect(screen.getByTestId('remote-indexer-settings-needed')).toBeInTheDocument();
	});

	it('renders engine auth busy and failures while preserving the handoff input for retry', async () => {
		runtime = await createAppRuntime();
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<RemoteSourceAcquireView />
			</AppRuntimeProvider>
		));
		fakeEngine().change((state) => {
			state.remote.providers = providerCapabilities();
			state.remote.account = {
				providerId: 'audible',
				status: 'needsAuth',
				message: null,
			};
			state.remote.busy = true;
		});
		await runtime.remoteSource.open({ lane: 'audible' });
		expect(screen.getByRole('button', { name: 'Connect Audible' })).toBeDisabled();
		expect(screen.getByRole('button', { name: 'Complete Auth' })).toBeDisabled();
		fireEvent.input(screen.getByLabelText('Auth Handoff'), {
			target: { value: '/auth/handoff' },
		});
		flush();
		fakeEngine().change((state) => {
			state.remote.audibleMessage = 'Authorization failed; retry.';
			state.remote.busy = false;
		});
		flush();
		expect(screen.getByRole('button', { name: 'Connect Audible' })).toBeEnabled();
		expect(screen.getByText('Authorization failed; retry.')).toBeInTheDocument();
		expect(screen.getByLabelText('Auth Handoff')).toHaveValue('/auth/handoff');
		fireEvent.click(screen.getByRole('button', { name: 'Complete Auth' }));
		flush();
		await vi.waitFor(() =>
			expect(fakeEngine().sessionIntents).toContainEqual({
				kind: 'remote',
				intent: { kind: 'completeAuth', responseUrlHandoffPath: '/auth/handoff' },
			}),
		);
	});

	it('shows connected Indexer search controls and searches on Enter in author or title', async () => {
		runtime = await createAppRuntime();
		const runAction = vi.spyOn(runtime.remoteSource, 'runAction');
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<RemoteSourceAcquireView />
			</AppRuntimeProvider>
		));
		await openConnected(runtime, 'indexer');
		await Promise.resolve();
		expect(screen.queryByTestId('remote-indexer-settings-needed')).not.toBeInTheDocument();
		expect(screen.getByTestId('remote-source-indexer-author')).toBeEnabled();
		expect(screen.getByRole('button', { name: 'Search' })).toBeEnabled();
		runAction.mockClear();

		fireEvent.keyDown(screen.getByTestId('remote-source-indexer-author'), { key: 'Enter' });
		flush();
		expect(runAction).toHaveBeenCalledWith({ type: 'searchReleases' });

		runAction.mockClear();
		fireEvent.keyDown(screen.getByTestId('remote-source-indexer-title'), { key: 'Enter' });
		flush();
		expect(runAction).toHaveBeenCalledWith({ type: 'searchReleases' });
	});

	it('sends the clicked release key and renders the engine’s selected and failed rows', async () => {
		const releases: RemoteRelease[] = [7, 8].map((indexerId) => ({
			providerId: 'indexer',
			key: `release-${indexerId}`,
			guid: 'same-guid',
			indexerId,
			title: `Release from ${indexerId}`,
			indexer: `Indexer ${indexerId}`,
			sizeBytes: 1000,
			protocol: 'torrent',
			detailUrl: null,
			seeders: indexerId === 8 ? 72 : 10,
			categories: [{ id: 3030, name: 'Audio/Audiobook' }],
		}));
		runtime = await createAppRuntime();
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<RemoteSourceAcquireView />
			</AppRuntimeProvider>
		));
		await openConnected(runtime, 'indexer', releases);
		expect(screen.getAllByText('torrent').length).toBeGreaterThan(0);
		expect(screen.getAllByText('Audio/Audiobook').length).toBeGreaterThan(0);
		expect(screen.getByText('Indexer 8')).toBeInTheDocument();
		expect(screen.getAllByText(/72 seeders/).length).toBeGreaterThan(0);
		fireEvent.click(screen.getByRole('button', { name: /^Select Release from 8/ }));
		flush();
		await vi.waitFor(() =>
			expect(fakeEngine().sessionIntents).toContainEqual({
				kind: 'remote',
				intent: { kind: 'selectRelease', key: 'release-8', multi: false },
			}),
		);
		fakeEngine().change((state) => {
			state.remote.indexer.selectedReleaseKeys = ['release-8'];
		});
		flush();
		expect(screen.getByRole('button', { name: /^Select Release from 8/ })).toHaveAttribute(
			'aria-pressed',
			'true',
		);
		const releaseButton = screen.getByRole('button', { name: /^Select Release from 8/ });
		fireEvent.click(screen.getByRole('button', { name: 'Grab Release from 8 from Indexer 8' }));
		flush();
		await vi.waitFor(() =>
			expect(fakeEngine().sessionIntents).toContainEqual({
				kind: 'remote',
				intent: { kind: 'grabRelease', key: 'release-8' },
			}),
		);
		fakeEngine().change((state) => {
			state.remote.indexer.releaseGrabs = {
				'release-8': { status: 'error', message: 'Downloader unavailable' },
			};
		});
		flush();
		expect(screen.getByText('Downloader unavailable')).toBeInTheDocument();
		expect(screen.getByRole('button', { name: /^Select Release from 8/ })).toBe(releaseButton);
		expect(
			screen.getByRole('button', { name: 'Retry Release from 8 from Indexer 8' }),
		).toBeEnabled();
	});
});

describe('RemoteSourceAcquireView during a download', () => {
	let runtime: AppRuntime | undefined;
	afterEach(() => {
		cleanup();
		runtime?.dispose();
		runtime = undefined;
		document.body.innerHTML = '';
	});

	function engineWithUpdates() {
		const engine = createFakeEngine();
		let deliver: ((update: SessionUpdate) => void) | undefined;
		let revision = 1_000_000;
		const capability = {
			...engine,
			listenSessionUpdates: (handler: (update: SessionUpdate) => void) => {
				deliver = handler;
				return engine.listenSessionUpdates(handler);
			},
		};
		return {
			engine,
			capability,
			progress(percentage: number) {
				if (!deliver) throw new Error('not attached yet');
				revision += 1;
				deliver(
					sessionUpdate({
						revision,
						remoteAcquisition: { revision, job: acquisitionJob(percentage) },
					}),
				);
				flush();
			},
		};
	}

	function seedLibrary(engine: ReturnType<typeof createFakeEngine>, count: number) {
		const titles = Array.from({ length: count }, (_, index) => ({
			...remoteTitle(),
			titleId: `B${String(index).padStart(9, '0')}`,
			title: `Book ${index}`,
			supplementalPdfAvailable: index % 5 === 0,
		}));
		engine.change((state) => {
			state.remote.lane = 'audible';
			state.remote.providers = providerCapabilities();
			state.remote.account = {
				providerId: 'audible',
				status: 'connected',
				message: null,
			};
			state.remote.selectedTitleIds = [titles[1].titleId];
			state.remote.includePdfByTitleId = Object.fromEntries(
				titles.map((title) => [title.titleId, title.supplementalPdfAvailable]),
			);
			state.remoteAcquisition.job = acquisitionJob(10);
			state.remoteLibrary.titles = titles;
		});
	}

	it('advances progress at 1,000 titles without re-running or rebuilding any row', async () => {
		const host = engineWithUpdates();
		seedLibrary(host.engine, 1000);
		runtime = await createAppRuntime({ engine: host.capability });
		await runtime.remoteSource.open({ lane: 'audible' });
		const owner = runtime.remoteSource;
		const reads = { row: 0, list: 0, view: 0 };
		const { isTitleSelected, includesPdf, visibleTitles, view } = owner;
		Object.assign(owner, {
			view: () => {
				reads.view += 1;
				return view();
			},
			isTitleSelected: (titleId: string) => {
				reads.row += 1;
				return isTitleSelected(titleId);
			},
			includesPdf: (titleId: string) => {
				reads.row += 1;
				return includesPdf(titleId);
			},
			visibleTitles: () => {
				reads.list += 1;
				return visibleTitles();
			},
		});
		const app = runtime;
		render(() => (
			<AppRuntimeProvider runtime={app}>
				<RemoteSourceAcquireView />
			</AppRuntimeProvider>
		));
		flush();
		const rows = [...document.querySelectorAll('.remote-title-row')];
		expect(rows).toHaveLength(1000);
		expect(reads.row).toBeGreaterThan(1000);
		reads.row = 0;
		reads.list = 0;
		reads.view = 0;

		host.progress(55);

		expect(screen.getByRole('progressbar', { name: 'Acquisition progress' })).toHaveAttribute(
			'aria-valuenow',
			'55',
		);
		expect({ row: reads.row, list: reads.list }).toEqual({ row: 0, list: 0 });
		// Only the dialog's own bindings read view(); a row reading it would add 1,000.
		expect(reads.view).toBeLessThan(100);
		const progress = screen.getByRole('progressbar', { name: 'Acquisition progress' });
		expect(progress.closest('[aria-live]')).toBeNull();
		const after = [...document.querySelectorAll('.remote-title-row')];
		expect(after.every((row, index) => row === rows[index])).toBe(true);
		expect(after[1]).toHaveAttribute('aria-selected', 'true');
	});

	it('keeps no title rows while closed and shows current choices on reopen', async () => {
		const host = engineWithUpdates();
		seedLibrary(host.engine, 20);
		runtime = await createAppRuntime({ engine: host.capability });
		await runtime.remoteSource.open({ lane: 'audible' });
		const app = runtime;
		render(() => (
			<AppRuntimeProvider runtime={app}>
				<RemoteSourceAcquireView />
			</AppRuntimeProvider>
		));
		flush();
		expect(document.querySelectorAll('.remote-title-row')).toHaveLength(20);
		runtime.remoteSource.editSearch({ titleFilter: 'Book 1' });
		runtime.remoteSource.close();
		flush();
		await new Promise((resolve) => setTimeout(resolve, 0));
		flush();
		expect(document.querySelectorAll('.remote-title-row')).toHaveLength(0);

		host.progress(80);
		host.engine.change((state) => {
			state.remote.selectedTitleIds = ['B000000010'];
		});
		await runtime.remoteSource.open({ lane: 'audible' });
		flush();
		expect(screen.getByLabelText('Filter')).toHaveValue('Book 1');
		expect(screen.getByRole('option', { name: /Book 10/ })).toHaveAttribute(
			'aria-selected',
			'true',
		);
		expect(document.querySelectorAll('.remote-title-row[aria-selected="true"]')).toHaveLength(1);
		expect(screen.getByRole('progressbar', { name: 'Acquisition progress' })).toBeInTheDocument();
	});

	it('reopens each list where it was scrolled when closed', async () => {
		const host = engineWithUpdates();
		seedLibrary(host.engine, 20);
		runtime = await createAppRuntime({ engine: host.capability });
		await runtime.remoteSource.open({ lane: 'audible' });
		const app = runtime;
		render(() => (
			<AppRuntimeProvider runtime={app}>
				<RemoteSourceAcquireView />
			</AppRuntimeProvider>
		));
		flush();
		const list = () => screen.getByRole('listbox', { name: 'Audible titles' });
		list().scrollTop = 400;
		fireEvent.scroll(list());
		flush();
		runtime.remoteSource.close();
		flush();
		await new Promise((resolve) => setTimeout(resolve, 0));
		flush();
		await runtime.remoteSource.open({ lane: 'audible' });
		flush();
		await vi.waitFor(() => expect(list().scrollTop).toBe(400));
	});

	it('re-runs no release row when a download tick arrives in the Indexer lane', async () => {
		const host = engineWithUpdates();
		host.engine.change((state) => {
			state.remote.lane = 'indexer';
			state.remote.providers = providerCapabilities();
			state.remote.account = {
				providerId: 'indexer',
				status: 'connected',
				message: null,
			};
			state.remote.indexer.releases = Array.from({ length: 40 }, (_, index) => ({
				providerId: 'indexer' as const,
				key: `guid-${index}`,
				guid: `guid-${index}`,
				indexerId: 1,
				title: `Release ${index}`,
				indexer: 'Indexer',
				sizeBytes: 1000,
				protocol: 'torrent' as const,
				detailUrl: null,
				seeders: index,
				categories: [],
			}));
			state.remoteAcquisition.job = acquisitionJob(10);
		});
		runtime = await createAppRuntime({ engine: host.capability });
		await runtime.remoteSource.open({ lane: 'indexer' });
		const owner = runtime.remoteSource;
		let views = 0;
		const { view } = owner;
		Object.assign(owner, {
			view: () => {
				views += 1;
				return view();
			},
		});
		const app = runtime;
		render(() => (
			<AppRuntimeProvider runtime={app}>
				<RemoteSourceAcquireView />
			</AppRuntimeProvider>
		));
		flush();
		expect(screen.getAllByRole('button', { name: /^Grab Release/ })).toHaveLength(40);
		views = 0;

		host.progress(60);

		// Only the dialog's own bindings read view(); a release row reading it would add 40.
		expect(views).toBeLessThan(40);
	});
});
