import { flush } from 'solid-js';
import { fakeEngine } from '../../test/fixtures/fakeEngine';
import { cleanup, fireEvent, render, screen } from '@solidjs/testing-library';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AppRuntimeProvider, createAppRuntime, type AppRuntime } from '../../app/runtime';

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
			currentTitleId: 'B000000001',
			currentItemIndex: 1,
			totalItems: 1,
		},
		materializedFiles: [],
		supplementalAssets: [],
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
		durationSeconds: 3600,
		supplementalPdfAvailable: false,
		acquired: false,
		availability: {
			status: 'available',
			acquirable: true,
			label: 'Available',
		},
		unsupportedReasons: [],
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
		state.remote.account = { providerId: lane, status: 'connected' };
		state.remote.libraryStatus = { kind: 'succeeded' };
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

describe('RemoteSourceAcquireView close wiring', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		cleanup();
		runtime?.dispose();
		runtime = undefined;
		vi.restoreAllMocks();
		document.body.innerHTML = '';
	});

	it('routes Escape through the same close callback the Close button uses without cancelling acquisition', async () => {
		runtime = createAppRuntime();
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<button type="button" id="acquire-invoker">
					Open
				</button>
				<RemoteSourceAcquireView />
			</AppRuntimeProvider>
		));
		void runtime.remoteSource.open();
		await Promise.resolve();

		await fireEvent.keyDown(document.getElementById('remote-source-close') as Element, {
			key: 'Escape',
			bubbles: true,
		});

		expect(runtime.remoteSource.view().isOpen).toBe(false);
	});

	it('renders engine acquisition snapshots only in the Audible lane', async () => {
		runtime = createAppRuntime();
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<RemoteSourceAcquireView />
			</AppRuntimeProvider>
		));
		await openConnected(runtime, 'audible');
		fakeEngine().change((state) => {
			state.remote.acquisition = acquisitionJob(40);
			state.remote.selectedTitleIds = [remoteTitle().titleId];
		});
		flush();
		expect(screen.getByRole('progressbar', { name: 'Acquisition progress' })).toHaveAttribute(
			'aria-valuenow',
			'40',
		);
		expect(screen.getByRole('option', { name: new RegExp(remoteTitle().title) })).toHaveAttribute(
			'aria-selected',
			'true',
		);
		expect(screen.getByRole('button', { name: 'Refresh Library' })).toBeDisabled();
		expect(screen.getByRole('button', { name: 'Cancel Acquisition' })).toBeEnabled();
		await fireEvent.click(screen.getByRole('button', { name: 'Cancel Acquisition' }));
		await vi.waitFor(() =>
			expect(fakeEngine().sessionIntents).toContainEqual({
				kind: 'remote',
				intent: { kind: 'cancelAcquisition', jobId: 'remote-job-1' },
			}),
		);
		fakeEngine().change((state) => {
			state.remote.acquisition = acquisitionJob(100, true);
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
		expect(screen.getByRole('progressbar', { name: 'Acquisition progress' })).toHaveAttribute(
			'aria-valuenow',
			'100',
		);
	});

	it('exposes Audible controls and filters connected library titles', async () => {
		runtime = createAppRuntime();
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
		await fireEvent.input(screen.getByLabelText('Filter'), {
			target: { value: 'no matching book' },
		});
		expect(screen.queryByText(remoteTitle().title)).not.toBeInTheDocument();
	});

	it('switches source lanes from the enabled provider control', async () => {
		runtime = createAppRuntime();
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
		runtime = createAppRuntime();
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<RemoteSourceAcquireView />
			</AppRuntimeProvider>
		));
		fakeEngine().change((state) => {
			state.remote.providers = providerCapabilities();
			state.remote.account = { providerId: 'audible', status: 'needsAuth' };
			state.remote.auth = { kind: 'starting' };
		});
		await runtime.remoteSource.open();
		expect(screen.getByRole('button', { name: 'Connect Audible' })).toBeDisabled();
		expect(screen.getByRole('button', { name: 'Complete Auth' })).toBeDisabled();
		await fireEvent.input(screen.getByLabelText('Auth Handoff'), {
			target: { value: '/auth/handoff' },
		});
		fakeEngine().change((state) => {
			state.remote.auth = {
				kind: 'failed',
				error: {
					category: 'io',
					code: 'io_error',
					message: 'Authorization failed; retry.',
					detail: null,
				},
			};
		});
		flush();
		expect(screen.getByRole('button', { name: 'Connect Audible' })).toBeEnabled();
		expect(screen.getByText('Authorization failed; retry.')).toBeInTheDocument();
		expect(screen.getByLabelText('Auth Handoff')).toHaveValue('/auth/handoff');
		await fireEvent.click(screen.getByRole('button', { name: 'Complete Auth' }));
		await vi.waitFor(() =>
			expect(fakeEngine().sessionIntents).toContainEqual({
				kind: 'remote',
				intent: { kind: 'completeAuth', responseUrlHandoffPath: '/auth/handoff' },
			}),
		);
	});

	it('shows indexer search controls when the lane is connected', async () => {
		runtime = createAppRuntime();
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
	});

	it('searches indexer releases when Enter is pressed in the author or title field', async () => {
		runtime = createAppRuntime();
		const runAction = vi.spyOn(runtime.remoteSource, 'runAction');
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<RemoteSourceAcquireView />
			</AppRuntimeProvider>
		));
		await openConnected(runtime, 'indexer');
		await Promise.resolve();
		runAction.mockClear();

		await fireEvent.keyDown(screen.getByTestId('remote-source-indexer-author'), { key: 'Enter' });
		expect(runAction).toHaveBeenCalledWith({ type: 'searchReleases' });

		runAction.mockClear();
		await fireEvent.keyDown(screen.getByTestId('remote-source-indexer-title'), { key: 'Enter' });
		expect(runAction).toHaveBeenCalledWith({ type: 'searchReleases' });
	});

	it('paints protocol, category, and indexer as release tags', async () => {
		runtime = createAppRuntime();
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<RemoteSourceAcquireView />
			</AppRuntimeProvider>
		));
		await openConnected(runtime, 'indexer', [
			{
				providerId: 'indexer',
				guid: 'extinction-1',
				indexerId: 7,
				title: 'Extinction by David Crouse [ENG / M4B]',
				indexer: 'MyAnonymouse',
				sizeBytes: 550_000_000,
				protocol: 'torrent',
				seeders: 72,
				categories: [{ id: 3030, name: 'Audio/Audiobook' }],
			},
		]);
		await Promise.resolve();

		expect(screen.getByText('torrent')).toHaveClass('remote-release-tag-torrent');
		expect(screen.getByText('Audio/Audiobook')).toHaveClass('remote-release-tag-category');
		expect(screen.getByText('MyAnonymouse')).toHaveClass('remote-release-tag-indexer');
		expect(screen.getByText(/72 seeders/)).toBeInTheDocument();
	});

	it('sends the clicked indexer identity and renders the engine’s selected and failed rows', async () => {
		const releases: RemoteRelease[] = [7, 8].map((indexerId) => ({
			providerId: 'indexer',
			guid: 'same-guid',
			indexerId,
			title: `Release from ${indexerId}`,
			indexer: `Indexer ${indexerId}`,
			sizeBytes: 1000,
			protocol: 'torrent',
			seeders: 10,
			categories: [],
		}));
		runtime = createAppRuntime();
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<RemoteSourceAcquireView />
			</AppRuntimeProvider>
		));
		await openConnected(runtime, 'indexer', releases);
		await fireEvent.click(screen.getByRole('button', { name: /^Select Release from 8/ }));
		await vi.waitFor(() =>
			expect(fakeEngine().sessionIntents).toContainEqual({
				kind: 'remote',
				intent: { kind: 'selectRelease', indexerId: 8, guid: 'same-guid', multi: false },
			}),
		);
		fakeEngine().change((state) => {
			state.remote.indexer.selectedReleaseKeys = ['[8,"same-guid"]'];
		});
		flush();
		expect(screen.getByRole('button', { name: /^Select Release from 8/ })).toHaveAttribute(
			'aria-pressed',
			'true',
		);
		await fireEvent.click(
			screen.getByRole('button', { name: 'Grab Release from 8 from Indexer 8' }),
		);
		await vi.waitFor(() =>
			expect(fakeEngine().sessionIntents).toContainEqual({
				kind: 'remote',
				intent: { kind: 'grabRelease', indexerId: 8, guid: 'same-guid' },
			}),
		);
		fakeEngine().change((state) => {
			state.remote.indexer.releaseGrabs = {
				'[8,"same-guid"]': { status: 'error', message: 'Downloader unavailable' },
			};
		});
		flush();
		expect(screen.getByText('Downloader unavailable')).toBeInTheDocument();
		expect(
			screen.getByRole('button', { name: 'Retry Release from 8 from Indexer 8' }),
		).toBeEnabled();
	});
});
