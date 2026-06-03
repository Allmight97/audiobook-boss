import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AcquisitionJob, RemoteSourceAccountState } from '../../types/remoteSource';
import RemoteSourceAcquireDialog from './RemoteSourceAcquireDialog.svelte';
import RemoteSourceAcquireIsland from './RemoteSourceAcquireIsland.svelte';
import { remoteSourceAcquireState } from './state.svelte';

const context = vi.hoisted(() => ({
	getRemoteSourceAccountStateMock: vi.fn(),
	loadRemoteSourceLibraryMock: vi.fn(),
	startRemoteSourceAcquisitionMock: vi.fn(),
	getRemoteSourceAcquisitionStatusMock: vi.fn(),
	purgeRemoteSourceSessionMock: vi.fn(),
	openUrlMock: vi.fn(),
	handleImportedAudioPathsMock: vi.fn(),
	getImportedAudioPathsBlockedMessageMock: vi.fn(),
	registerRemoteSourceSupplementalAssetsMock: vi.fn(),
	getCurrentFileListMock: vi.fn(),
}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		getRemoteSourceAccountState: context.getRemoteSourceAccountStateMock,
		loadRemoteSourceLibrary: context.loadRemoteSourceLibraryMock,
		startRemoteSourceAcquisition: context.startRemoteSourceAcquisitionMock,
		getRemoteSourceAcquisitionStatus: context.getRemoteSourceAcquisitionStatusMock,
		purgeRemoteSourceSession: context.purgeRemoteSourceSessionMock,
		openUrl: context.openUrlMock,
	},
}));

vi.mock('../fileImport/handlers', () => ({
	getImportedAudioPathsBlockedMessage: context.getImportedAudioPathsBlockedMessageMock,
	handleImportedAudioPaths: context.handleImportedAudioPathsMock,
}));

vi.mock('../fileList/state.svelte', () => ({
	getCurrentFileList: context.getCurrentFileListMock,
}));

vi.mock('./sessionAssets.svelte', () => ({
	registerRemoteSourceSupplementalAssets: context.registerRemoteSourceSupplementalAssetsMock,
}));

function connectedAccount(): RemoteSourceAccountState {
	return {
		providerId: 'audible',
		status: 'connected',
		account: {
			providerId: 'audible',
			accountId: 'audible-us',
			displayName: 'Audible account',
		},
		message: undefined,
	};
}

function acquisitionJob(overrides: Partial<AcquisitionJob> = {}): AcquisitionJob {
	return {
		jobId: 'remote-job-1',
		providerId: 'audible',
		status: 'acquiring',
		materializedFiles: [],
		supplementalAssets: [],
		diagnostics: [],
		progress: {
			stage: 'download',
			percentage: 35,
			message: 'Downloading audiobook.',
			bytesDownloaded: 50,
			bytesTotal: 100,
			terminal: false,
		},
		...overrides,
	} as AcquisitionJob;
}

describe('RemoteSourceAcquireIsland progress', () => {
	beforeEach(() => {
		remoteSourceAcquireState.isOpen = false;
		context.getRemoteSourceAccountStateMock.mockReset();
		context.getRemoteSourceAccountStateMock.mockResolvedValue(connectedAccount());
		context.loadRemoteSourceLibraryMock.mockReset();
		context.loadRemoteSourceLibraryMock.mockResolvedValue({
			providerId: 'audible',
			titles: [
				{
					providerId: 'audible',
					titleId: 'B000000001',
					title: 'Mock Audible Book',
					authors: ['Mock Author'],
					narrators: [],
					durationSeconds: 3600,
					coverUrl: undefined,
					supplementalPdfAvailable: false,
					acquired: false,
					unsupportedReasons: [],
				},
			],
			diagnostics: [],
		});
		context.startRemoteSourceAcquisitionMock.mockReset();
		context.startRemoteSourceAcquisitionMock.mockResolvedValue(acquisitionJob());
		context.getRemoteSourceAcquisitionStatusMock.mockReset();
		context.getRemoteSourceAcquisitionStatusMock
			.mockResolvedValueOnce(acquisitionJob())
			.mockResolvedValueOnce(
				acquisitionJob({
					progress: {
						stage: 'decryption',
						percentage: 75,
						message: 'Decrypting audiobook.',
						bytesDownloaded: undefined,
						bytesTotal: undefined,
						terminal: false,
					},
				}),
			)
			.mockResolvedValueOnce(
				acquisitionJob({
					status: 'failed',
					progress: {
						stage: 'failed',
						percentage: 100,
						message: 'Acquisition failed.',
						bytesDownloaded: undefined,
						bytesTotal: undefined,
						terminal: true,
					},
					diagnostics: [
						{
							kind: 'protectedUnsupported',
							titleId: 'B000000001',
							message: 'Audible decryption is not available for this encrypted asset.',
						},
					],
				}),
			);
		context.purgeRemoteSourceSessionMock.mockReset();
		context.purgeRemoteSourceSessionMock.mockResolvedValue(null);
		context.openUrlMock.mockReset();
		context.handleImportedAudioPathsMock.mockReset();
		context.handleImportedAudioPathsMock.mockResolvedValue({ status: 'imported' });
		context.getImportedAudioPathsBlockedMessageMock.mockReset();
		context.getImportedAudioPathsBlockedMessageMock.mockReturnValue(null);
		context.registerRemoteSourceSupplementalAssetsMock.mockReset();
		context.getCurrentFileListMock.mockReset();
	});

	afterEach(() => {
		vi.useRealTimers();
		remoteSourceAcquireState.isOpen = false;
	});

	it('keeps the input panel surface to a trigger button', async () => {
		const user = userEvent.setup();
		render(RemoteSourceAcquireIsland);

		await user.click(screen.getByRole('button', { name: 'Acquire' }));

		expect(remoteSourceAcquireState.isOpen).toBe(true);
	});

	it('polls acquisition status and renders active download/decrypt progress in the app modal', async () => {
		const user = userEvent.setup();
		remoteSourceAcquireState.isOpen = true;
		render(RemoteSourceAcquireDialog);

		await screen.findByText('Mock Audible Book');
		await user.click(screen.getByRole('button', { name: /Mock Audible Book/i }));

		vi.useFakeTimers();
		await fireEvent.click(screen.getByRole('button', { name: 'Acquire Selected' }));
		await tick();

		expect(screen.getAllByText('Downloading audiobook.').length).toBeGreaterThan(0);

		await vi.advanceTimersByTimeAsync(100);
		await tick();
		await waitFor(() => {
			expect(context.getRemoteSourceAcquisitionStatusMock).toHaveBeenCalledWith('remote-job-1');
		});
		await vi.advanceTimersByTimeAsync(100);
		await tick();
		expect(screen.getAllByText('Decrypting audiobook.').length).toBeGreaterThan(0);
		await vi.advanceTimersByTimeAsync(100);
		await tick();
		expect(screen.getAllByText('Acquisition failed.').length).toBeGreaterThan(0);
		expect(screen.getByText(/Audible decryption is not available/)).toBeTruthy();
		expect(document.body.textContent).not.toContain('fake-secret');
	});

	it('redacts unknown provider-boundary errors from status text and console logs', async () => {
		const consoleError = vi.spyOn(console, 'error').mockImplementation(() => undefined);
		context.getRemoteSourceAccountStateMock.mockRejectedValueOnce(
			new Error('token=fake-secret license=fake-license'),
		);
		remoteSourceAcquireState.isOpen = true;

		try {
			render(RemoteSourceAcquireDialog);

			await screen.findByText('Failed to load remote source state.');
			const consoleOutput = consoleError.mock.calls.flat().join(' ');
			expect(document.body.textContent).not.toContain('fake-secret');
			expect(document.body.textContent).not.toContain('fake-license');
			expect(consoleOutput).toContain('Failed to load remote source state.');
			expect(consoleOutput).toContain('code=unknown_error');
			expect(consoleOutput).not.toContain('fake-secret');
			expect(consoleOutput).not.toContain('fake-license');
		} finally {
			consoleError.mockRestore();
		}
	});

	it('does not start acquisition while file import handoff is order-locked', async () => {
		const user = userEvent.setup();
		context.getImportedAudioPathsBlockedMessageMock.mockReturnValue(
			'Order locked while processing. Wait for completion to add files.',
		);
		remoteSourceAcquireState.isOpen = true;
		render(RemoteSourceAcquireDialog);

		await screen.findByText('Mock Audible Book');
		await user.click(screen.getByRole('button', { name: /Mock Audible Book/i }));
		await user.click(screen.getByRole('button', { name: 'Acquire Selected' }));

		expect(context.startRemoteSourceAcquisitionMock).not.toHaveBeenCalled();
		expect(
			screen.getByText('Order locked while processing. Wait for completion to add files.'),
		).toBeTruthy();
	});

	it('does not report import success when file import handoff is blocked after acquisition', async () => {
		const user = userEvent.setup();
		context.startRemoteSourceAcquisitionMock.mockResolvedValueOnce(
			acquisitionJob({
				status: 'validated',
				materializedFiles: [
					{
						inputId: 'input-1',
						titleId: 'B000000001',
						path: '/tmp/remote/book.m4b',
						sizeBytes: 42,
						sha256: 'abc123',
					},
				],
				progress: {
					stage: 'importHandoff',
					percentage: 100,
					message: 'Ready for import.',
					bytesDownloaded: undefined,
					bytesTotal: undefined,
					terminal: true,
				},
			}),
		);
		context.handleImportedAudioPathsMock.mockResolvedValueOnce({
			status: 'blocked',
			message: 'Order locked while processing. Wait for completion to add files.',
		});
		remoteSourceAcquireState.isOpen = true;
		render(RemoteSourceAcquireDialog);

		await screen.findByText('Mock Audible Book');
		await user.click(screen.getByRole('button', { name: /Mock Audible Book/i }));
		await user.click(screen.getByRole('button', { name: 'Acquire Selected' }));

		await screen.findByText(/Order locked while processing\. Wait for completion to add files\./);
		expect(context.handleImportedAudioPathsMock).toHaveBeenCalledWith(['/tmp/remote/book.m4b']);
		expect(context.purgeRemoteSourceSessionMock).toHaveBeenCalledWith('remote-job-1');
		expect(context.registerRemoteSourceSupplementalAssetsMock).not.toHaveBeenCalled();
		expect(document.body.textContent).not.toContain('acquired title imported');
		expect(document.body.textContent).toContain('Staged remote files were removed');
	});
});
