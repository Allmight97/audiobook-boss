import { cleanup, fireEvent, render, screen } from '@solidjs/testing-library';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { resetRemoteSource } from '../../app/remoteSource';
import { AppRuntimeProvider } from '../../app/runtime/RuntimeProvider';
import { createTestAppRuntime } from '../../app/runtime/harness';
import type { AppRuntime } from '../../app/runtime';
import { tauriClient } from '../../lib/tauri/client';
import type { AcquisitionJob, RemoteTitle } from '../../types/remoteSource';
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
		resetRemoteSource();
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
});
