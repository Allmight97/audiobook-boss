import { cleanup, fireEvent, render, screen } from '@solidjs/testing-library';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AppRuntimeProvider } from '../../app/runtime/RuntimeProvider';
import { createTestAppRuntime } from '../../app/runtime/harness';
import type { AppRuntime } from '../../app/runtime';
import { tauriClient } from '../../lib/tauri/client';
import type {
	AcquisitionJob,
	RemoteSourceProviderCapabilities,
	RemoteTitle,
	RemoteRelease,
} from '../../types/remoteSource';
import { RemoteSourceAcquireView } from './RemoteSourceAcquireView';

function createDeferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((resolvePromise) => {
		resolve = resolvePromise;
	});
	return { promise, resolve };
}

function acquisitionJob(percentage: number, terminal = false): AcquisitionJob {
	return {
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
			authFlow: 'externalBrowserHandoff',
			supportsLibraryScan: true,
			supportsPagedScan: false,
			supportsTypeaheadFilter: true,
			supportsSupplementalPdf: true,
			supportsMaterializedAudio: true,
			supportsReleaseSearch: false,
			supportsReleaseGrab: false,
			supportsRefresh: true,
			requiresLiveSession: true,
			knownUnsupportedReasons: [],
		},
		{
			providerId: 'indexer',
			label: 'Indexer',
			authFlow: 'apiKey',
			supportsLibraryScan: false,
			supportsPagedScan: false,
			supportsTypeaheadFilter: false,
			supportsSupplementalPdf: false,
			supportsMaterializedAudio: false,
			supportsReleaseSearch: true,
			supportsReleaseGrab: true,
			supportsRefresh: false,
			requiresLiveSession: false,
			knownUnsupportedReasons: ['indexerConnectionRequired'],
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
	vi.spyOn(tauriClient, 'listRemoteSourceProviders').mockResolvedValue(providerCapabilities());
	vi.spyOn(tauriClient, 'getRemoteSourceAccountState').mockImplementation(async (providerId) => ({
		providerId,
		status: 'connected',
	}));
	vi.spyOn(tauriClient, 'loadRemoteSourceLibrary').mockResolvedValue({
		providerId: 'audible',
		titles: [remoteTitle()],
		diagnostics: [],
	});
	await runtime.remoteSource.open({ lane });
	if (releases) {
		vi.spyOn(tauriClient, 'searchRemoteSourceReleases').mockResolvedValue({
			providerId: 'indexer',
			releases,
			diagnostics: [],
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
		runtime = createTestAppRuntime();
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<button type="button" id="acquire-invoker">
					Open
				</button>
				<RemoteSourceAcquireView />
			</AppRuntimeProvider>
		));
		runtime.remoteSource.open();
		await Promise.resolve();

		await fireEvent.keyDown(document.getElementById('remote-source-close') as Element, {
			key: 'Escape',
			bubbles: true,
		});

		expect(runtime.remoteSource.view().isOpen).toBe(false);
	});

	it('renders polled acquisition progress published through the owner view', async () => {
		const terminalStatus = createDeferred<AcquisitionJob>();
		vi.spyOn(tauriClient, 'startRemoteSourceAcquisition').mockResolvedValue(acquisitionJob(10));
		vi.spyOn(tauriClient, 'getRemoteSourceAcquisitionStatus')
			.mockResolvedValueOnce(acquisitionJob(40))
			.mockImplementationOnce(() => terminalStatus.promise);

		runtime = createTestAppRuntime();
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<RemoteSourceAcquireView />
			</AppRuntimeProvider>
		));
		await openConnected(runtime, 'audible');
		runtime.remoteSource.toggleTitle('B000000001');
		await Promise.resolve();

		await fireEvent.click(screen.getByRole('button', { name: 'Acquire Selected' }));
		await vi.waitFor(() =>
			expect(screen.getByRole('progressbar', { name: 'Acquisition progress' })).toHaveAttribute(
				'aria-valuenow',
				'40',
			),
		);
		expect(screen.getByRole('button', { name: 'Cancel Acquisition' })).toBeEnabled();

		terminalStatus.resolve(acquisitionJob(100, true));
		await vi.waitFor(() => expect(runtime!.remoteSource.view().isBusy).toBe(false));
	});

	it('exposes Audible controls and filters connected library titles', async () => {
		runtime = createTestAppRuntime();
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
		vi.spyOn(tauriClient, 'listRemoteSourceProviders').mockResolvedValue(providerCapabilities());
		vi.spyOn(tauriClient, 'getRemoteSourceAccountState').mockResolvedValue({
			providerId: 'indexer',
			status: 'needsAuth',
			message: 'Configure Indexer URL and API key in Settings before searching.',
		});

		runtime = createTestAppRuntime();
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<RemoteSourceAcquireView />
			</AppRuntimeProvider>
		));
		await openConnected(runtime, 'audible');
		vi.mocked(tauriClient.getRemoteSourceAccountState).mockResolvedValue({
			providerId: 'indexer',
			status: 'needsAuth',
			message: 'Configure Indexer URL and API key in Settings before searching.',
		});
		await Promise.resolve();

		const user = userEvent.setup();
		await user.selectOptions(screen.getByTestId('remote-source-provider'), 'indexer');
		await Promise.resolve();

		await vi.waitFor(() => expect(runtime!.remoteSource.view().providerId).toBe('indexer'), {
			timeout: 2000,
		});
		expect(screen.getByTestId('remote-indexer-settings-needed')).toBeInTheDocument();
	});

	it('shows indexer search controls when the lane is connected', async () => {
		runtime = createTestAppRuntime();
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
		runtime = createTestAppRuntime();
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
		runtime = createTestAppRuntime();
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

	it('grabs the clicked indexer when release GUIDs match across indexers', async () => {
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
		const grab = vi.spyOn(tauriClient, 'grabRemoteSourceRelease').mockResolvedValue({
			providerId: 'indexer',
			accepted: true,
			message: 'Queued externally.',
			diagnostics: [],
		});
		runtime = createTestAppRuntime();
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<RemoteSourceAcquireView />
			</AppRuntimeProvider>
		));
		await openConnected(runtime, 'indexer', releases);
		await fireEvent.click(screen.getByRole('button', { name: /Release from 8/ }));
		expect(screen.getByRole('button', { name: /Release from 7/ })).toHaveAttribute(
			'aria-pressed',
			'false',
		);
		expect(screen.getByRole('button', { name: /Release from 8/ })).toHaveAttribute(
			'aria-pressed',
			'true',
		);
		await fireEvent.click(screen.getByRole('button', { name: 'Grab' }));
		await vi.waitFor(() => expect(grab).toHaveBeenCalledWith({ release: releases[1] }));
	});
	it('sorts loaded releases, opens source details, and preserves manual follow-up after a failed grab', async () => {
		const releases: RemoteRelease[] = [
			{
				providerId: 'indexer',
				guid: 'popular',
				indexerId: 19,
				title: 'Holmes single',
				indexer: 'MyAnonamouse',
				sizeBytes: 100,
				protocol: 'torrent',
				seeders: 80,
				categories: [],
			},
			{
				providerId: 'indexer',
				guid: 'collection',
				indexerId: 20,
				title: 'Holmes collection',
				indexer: 'AudioBookBay (Jackett)',
				sizeBytes: 900,
				protocol: 'torrent',
				seeders: 1,
				detailUrl: 'https://example.test/books/holmes',
				categories: [{ id: 3000, name: 'Audio' }],
			},
		];
		const openUrl = vi.spyOn(tauriClient, 'openUrl').mockResolvedValue(undefined);
		const grab = vi.spyOn(tauriClient, 'grabRemoteSourceRelease').mockResolvedValue({
			providerId: 'indexer',
			accepted: false,
			message: 'Indexer could not grab this release.',
			diagnostics: [],
		});
		runtime = createTestAppRuntime();
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<RemoteSourceAcquireView />
			</AppRuntimeProvider>
		));
		await openConnected(runtime, 'indexer', releases);
		const rows = () => screen.getAllByRole('button', { name: /Holmes/ });
		expect(rows()[0]).toHaveTextContent('Holmes single');
		await fireEvent.click(rows()[1]);
		await fireEvent.change(screen.getByLabelText('Sort by'), { target: { value: 'size' } });
		expect(rows()[0]).toHaveTextContent('Holmes collection');
		expect(rows()[0]).toHaveAttribute('aria-pressed', 'true');
		await fireEvent.input(screen.getByLabelText('Filter'), { target: { value: 'collection' } });
		expect(rows()).toHaveLength(1);
		const details = screen.getByRole('link', { name: 'View details for Holmes collection' });
		await fireEvent.click(details);
		expect(openUrl).toHaveBeenCalledExactlyOnceWith(releases[1].detailUrl);
		expect(runtime.remoteSource.view().selectedRelease).toEqual({
			guid: 'collection',
			indexerId: 20,
		});
		await fireEvent.click(screen.getByRole('button', { name: 'Grab' }));
		await vi.waitFor(() =>
			expect(screen.getByText('Indexer could not grab this release.')).toBeInTheDocument(),
		);
		expect(grab).toHaveBeenCalledExactlyOnceWith({ release: releases[1] });
		expect(details).toHaveAttribute('href', releases[1].detailUrl);
		await fireEvent.click(details);
		expect(openUrl).toHaveBeenCalledTimes(2);
		expect(screen.getByLabelText('Filter')).toHaveValue('collection');
		await fireEvent.input(screen.getByLabelText('Filter'), { target: { value: '' } });
		await fireEvent.change(screen.getByLabelText('Sort by'), { target: { value: 'seeders' } });
		expect(rows()[0]).toHaveTextContent('Holmes single');
		expect(screen.getAllByRole('link', { name: /View details/ })).toHaveLength(1);
		openUrl.mockRejectedValueOnce(new Error('Browser unavailable'));
		await fireEvent.click(details);
		await vi.waitFor(() =>
			expect(screen.getByText('Could not open the source page.')).toBeInTheDocument(),
		);
		expect(runtime.remoteSource.view().isOpen).toBe(true);
	});
});
