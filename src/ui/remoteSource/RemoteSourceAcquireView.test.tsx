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
		runtime.remoteSource.patch({
			isOpen: true,
			didHydrateOpenDialog: true,
			accountState: { providerId: 'audible', status: 'connected' },
			titles: [remoteTitle()],
			selectedTitleIds: new Set(['B000000001']),
		});
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
		runtime.remoteSource.patch({
			isOpen: true,
			didHydrateOpenDialog: true,
			providerId: 'audible',
			providers: providerCapabilities(),
			accountState: { providerId: 'audible', status: 'connected' },
			titles: [remoteTitle()],
		});
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
		runtime.remoteSource.patch({
			isOpen: true,
			didHydrateOpenDialog: true,
			providerId: 'audible',
			providers: providerCapabilities(),
			accountState: { providerId: 'audible', status: 'connected' },
			titles: [remoteTitle()],
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
		runtime.remoteSource.patch({
			isOpen: true,
			didHydrateOpenDialog: true,
			providerId: 'indexer',
			providers: providerCapabilities(),
			accountState: { providerId: 'indexer', status: 'connected' },
		});
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
		runtime.remoteSource.patch({
			isOpen: true,
			didHydrateOpenDialog: true,
			providerId: 'indexer',
			providers: providerCapabilities(),
			accountState: { providerId: 'indexer', status: 'connected' },
		});
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
		runtime.remoteSource.patch({
			isOpen: true,
			didHydrateOpenDialog: true,
			providerId: 'indexer',
			providers: providerCapabilities(),
			accountState: { providerId: 'indexer', status: 'connected' },
			releases: [
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
			],
		});
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
		runtime.remoteSource.patch({
			isOpen: true,
			didHydrateOpenDialog: true,
			providerId: 'indexer',
			providers: providerCapabilities(),
			accountState: { providerId: 'indexer', status: 'connected' },
			releases,
		});
		await fireEvent.click(screen.getByRole('button', { name: /Release from 8/ }));
		expect(screen.getByRole('option', { name: /Release from 7/ })).toHaveAttribute(
			'aria-selected',
			'false',
		);
		expect(screen.getByRole('option', { name: /Release from 8/ })).toHaveAttribute(
			'aria-selected',
			'true',
		);
		await fireEvent.click(screen.getByRole('button', { name: 'Grab' }));
		await vi.waitFor(() => expect(grab).toHaveBeenCalledWith({ release: releases[1] }));
	});
});
