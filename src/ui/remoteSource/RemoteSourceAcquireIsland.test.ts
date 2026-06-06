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
			currentTitleId: 'B000000001',
			currentItemIndex: 1,
			totalItems: 1,
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
						currentTitleId: 'B000000001',
						currentItemIndex: 1,
						totalItems: 1,
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
						currentTitleId: 'B000000001',
						currentItemIndex: 1,
						totalItems: 1,
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

		await user.click(screen.getByRole('button', { name: 'Import from Library' }));

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
		expect(screen.getByText('Downloading audiobook: 1/1 - Mock Audible Book')).toBeTruthy();

		await vi.advanceTimersByTimeAsync(100);
		await tick();
		await waitFor(() => {
			expect(context.getRemoteSourceAcquisitionStatusMock).toHaveBeenCalledWith('remote-job-1');
		});
		await vi.advanceTimersByTimeAsync(100);
		await tick();
		expect(screen.getAllByText('Decrypting audiobook.').length).toBeGreaterThan(0);
		expect(screen.getByText('Decrypting audiobook: 1/1 - Mock Audible Book')).toBeTruthy();
		await vi.advanceTimersByTimeAsync(100);
		await tick();
		expect(screen.getByText('Acquisition failed: 1/1 - Mock Audible Book')).toBeTruthy();
		expect(screen.getByText(/Audible decryption is not available/)).toBeTruthy();
		expect(document.body.textContent).not.toContain('fake-secret');
	});

	it('shows selected title count, hidden filtered selections, and clear selection action', async () => {
		const user = userEvent.setup();
		context.loadRemoteSourceLibraryMock.mockResolvedValueOnce({
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
				{
					providerId: 'audible',
					titleId: 'B000000002',
					title: 'Hidden Selection',
					authors: ['Second Author'],
					narrators: [],
					durationSeconds: 4200,
					coverUrl: undefined,
					supplementalPdfAvailable: false,
					acquired: false,
					unsupportedReasons: [],
				},
			],
			diagnostics: [],
		});
		remoteSourceAcquireState.isOpen = true;
		render(RemoteSourceAcquireDialog);

		const acquireButton = await screen.findByRole('button', { name: 'Acquire Selected' });
		expect((acquireButton as HTMLButtonElement).disabled).toBe(true);
		expect(screen.getByText('0 selected')).toBeTruthy();

		await user.click(await screen.findByText('Hidden Selection'));
		expect(screen.getByText('1 title selected')).toBeTruthy();
		expect((acquireButton as HTMLButtonElement).disabled).toBe(false);

		await user.type(screen.getByPlaceholderText('Filter loaded titles'), 'Mock');
		expect(screen.getByText('1 title selected (1 title hidden by filter)')).toBeTruthy();

		await user.click(screen.getByRole('button', { name: 'Clear selection' }));
		expect(screen.getByText('0 selected')).toBeTruthy();
		expect((acquireButton as HTMLButtonElement).disabled).toBe(true);
	});

	it('filters loaded titles to Supplemental PDF titles as a separate facet', async () => {
		const user = userEvent.setup();
		context.loadRemoteSourceLibraryMock.mockResolvedValueOnce({
			providerId: 'audible',
			titles: [
				{
					providerId: 'audible',
					titleId: 'B000000001',
					title: 'Standard Audible Book',
					authors: ['General Author'],
					narrators: [],
					durationSeconds: 3600,
					coverUrl: undefined,
					supplementalPdfAvailable: false,
					acquired: false,
					unsupportedReasons: [],
				},
				{
					providerId: 'audible',
					titleId: 'B000000002',
					title: 'Companion Guide',
					authors: ['Bob Example'],
					narrators: [],
					durationSeconds: 4200,
					coverUrl: undefined,
					supplementalPdfAvailable: true,
					acquired: false,
					unsupportedReasons: [],
				},
				{
					providerId: 'audible',
					titleId: 'B000000003',
					title: 'Supplemental Workbook',
					authors: ['Another Author'],
					narrators: [],
					durationSeconds: 4800,
					coverUrl: undefined,
					supplementalPdfAvailable: true,
					acquired: false,
					unsupportedReasons: [],
				},
			],
			diagnostics: [],
		});
		remoteSourceAcquireState.isOpen = true;
		render(RemoteSourceAcquireDialog);

		await screen.findByText('Standard Audible Book');
		await user.click(screen.getByRole('button', { name: /Standard Audible Book/i }));
		await user.click(screen.getByRole('checkbox', { name: 'Supplemental PDF only' }));

		expect(screen.queryByText('Standard Audible Book')).toBeNull();
		expect(screen.getByText('Companion Guide')).toBeTruthy();
		expect(screen.getByText('Supplemental Workbook')).toBeTruthy();
		expect(screen.getByText('1 title selected (1 title hidden by filter)')).toBeTruthy();

		await user.type(screen.getByPlaceholderText('Filter loaded titles'), 'Bob');
		expect(screen.getByText('Companion Guide')).toBeTruthy();
		expect(screen.queryByText('Supplemental Workbook')).toBeNull();
	});

	it('shows non-playable Audible titles but does not allow selecting them for acquisition', async () => {
		const user = userEvent.setup();
		context.loadRemoteSourceLibraryMock.mockResolvedValueOnce({
			providerId: 'audible',
			titles: [
				{
					providerId: 'audible',
					titleId: 'B000000001',
					title: 'Subscription Visible Book',
					authors: ['Visible Author'],
					narrators: [],
					durationSeconds: 3600,
					coverUrl: undefined,
					supplementalPdfAvailable: false,
					acquired: false,
					availability: {
						status: 'catalogOnly',
						acquirable: false,
						label: 'Audible catalog title',
						detail: 'Audible reports this title is not downloadable for this account.',
					},
					unsupportedReasons: ['protectedUnsupported'],
				},
			],
			diagnostics: [],
		});
		remoteSourceAcquireState.isOpen = true;
		render(RemoteSourceAcquireDialog);

		await screen.findByText('Subscription Visible Book');
		expect(screen.getByText('Audible catalog title')).toBeTruthy();
		expect(
			screen.getByText('Audible reports this title is not downloadable for this account.'),
		).toBeTruthy();
		const titleButton = screen.getByRole('button', { name: /Subscription Visible Book/i });
		expect((titleButton as HTMLButtonElement).disabled).toBe(true);

		await user.click(titleButton);
		expect(screen.getByText('0 selected')).toBeTruthy();
		expect(
			(screen.getByRole('button', { name: 'Acquire Selected' }) as HTMLButtonElement).disabled,
		).toBe(true);

		await user.click(screen.getByRole('checkbox', { name: 'Hide unavailable' }));
		expect(screen.queryByText('Subscription Visible Book')).toBeNull();
	});

	it('clears stale terminal acquisition copy when a fresh acquisition starts', async () => {
		const user = userEvent.setup();
		context.startRemoteSourceAcquisitionMock.mockResolvedValueOnce(
			acquisitionJob({
				status: 'failed',
				progress: {
					stage: 'failed',
					percentage: 100,
					message: 'Acquisition failed.',
					bytesDownloaded: undefined,
					bytesTotal: undefined,
					currentTitleId: 'B000000001',
					currentItemIndex: 1,
					totalItems: 1,
					terminal: true,
				},
				diagnostics: [
					{
						kind: 'downloadFailed',
						titleId: 'B000000001',
						message: 'Previous download failed.',
					},
				],
			}),
		);
		context.startRemoteSourceAcquisitionMock.mockImplementationOnce(
			() => new Promise(() => undefined),
		);
		remoteSourceAcquireState.isOpen = true;
		render(RemoteSourceAcquireDialog);

		await screen.findByText('Mock Audible Book');
		await user.click(screen.getByRole('button', { name: /Mock Audible Book/i }));
		await user.click(screen.getByRole('button', { name: 'Acquire Selected' }));

		await screen.findByText('Previous download failed.');

		await user.click(screen.getByRole('button', { name: 'Acquire Selected' }));
		await tick();

		expect(screen.getByText('Starting Audible acquisition.')).toBeTruthy();
		expect(document.body.textContent).not.toContain('Previous download failed.');
	});

	it('deduplicates repeated safe diagnostic copy for batch acquisition failures', async () => {
		const user = userEvent.setup();
		const repeatedMessage = 'Audible license response did not include a downloadable audio URL.';
		context.startRemoteSourceAcquisitionMock.mockResolvedValueOnce(
			acquisitionJob({
				status: 'failed',
				progress: {
					stage: 'failed',
					percentage: 100,
					message: 'Acquisition failed.',
					bytesDownloaded: undefined,
					bytesTotal: undefined,
					currentTitleId: 'B000000001',
					currentItemIndex: 1,
					totalItems: 1,
					terminal: true,
				},
				diagnostics: [
					{
						kind: 'providerPrivateProtocolFailed',
						titleId: 'B000000001',
						message: repeatedMessage,
					},
					{
						kind: 'providerPrivateProtocolFailed',
						titleId: 'B000000002',
						message: repeatedMessage,
					},
				],
			}),
		);
		remoteSourceAcquireState.isOpen = true;
		render(RemoteSourceAcquireDialog);

		await screen.findByText('Mock Audible Book');
		await user.click(screen.getByRole('button', { name: /Mock Audible Book/i }));
		await user.click(screen.getByRole('button', { name: 'Acquire Selected' }));

		await screen.findByText(repeatedMessage);
		const occurrenceCount = (document.body.textContent?.split(repeatedMessage).length ?? 1) - 1;
		expect(occurrenceCount).toBe(1);
	});

	it('does not import or register assets when required Supplemental PDF acquisition fails', async () => {
		const user = userEvent.setup();
		const message =
			'Audible Supplemental PDF could not be downloaded. The requested Supplemental PDF is required for this Audible title.';
		context.startRemoteSourceAcquisitionMock.mockResolvedValueOnce(
			acquisitionJob({
				status: 'failed',
				materializedFiles: [],
				supplementalAssets: [],
				progress: {
					stage: 'failed',
					percentage: 100,
					message: 'Acquisition failed.',
					bytesDownloaded: undefined,
					bytesTotal: undefined,
					currentTitleId: 'B000000001',
					currentItemIndex: 1,
					totalItems: 1,
					terminal: true,
				},
				diagnostics: [
					{
						kind: 'supplementalPdfFailed',
						titleId: 'B000000001',
						message,
					},
				],
			}),
		);
		remoteSourceAcquireState.isOpen = true;
		render(RemoteSourceAcquireDialog);

		await screen.findByText('Mock Audible Book');
		await user.click(screen.getByRole('button', { name: /Mock Audible Book/i }));
		await user.click(screen.getByRole('button', { name: 'Acquire Selected' }));

		await screen.findByText(message);
		expect(context.handleImportedAudioPathsMock).not.toHaveBeenCalled();
		expect(context.registerRemoteSourceSupplementalAssetsMock).not.toHaveBeenCalled();
		expect(context.purgeRemoteSourceSessionMock).not.toHaveBeenCalled();
		expect(document.body.textContent).not.toContain('acquired title imported');
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

	it('registers supplemental PDF assets after successful remote import handoff', async () => {
		const user = userEvent.setup();
		const terminalJob = acquisitionJob({
			status: 'validated',
			materializedFiles: [
				{
					inputId: 'provider-input-1',
					titleId: 'B000000001',
					path: '/tmp/remote/book.m4b',
					sizeBytes: 42,
					sha256: 'abc123',
				},
			],
			supplementalAssets: [
				{
					assetId: 'pdf-1',
					inputId: 'provider-input-1',
					titleId: 'B000000001',
					path: '/tmp/remote/book.pdf',
					fileName: 'Being You - A New Science of Consciousness - Supplemental PDF.pdf',
					sizeBytes: 32,
					sha256: 'pdf-sha',
				},
			],
			progress: {
				stage: 'importHandoff',
				percentage: 100,
				message: 'Ready for import.',
				bytesDownloaded: undefined,
				bytesTotal: undefined,
				currentTitleId: 'B000000001',
				currentItemIndex: 1,
				totalItems: 1,
				terminal: true,
			},
		});
		const currentFileList = {
			files: [
				{
					inputId: 'current-input-1',
					path: '/tmp/remote/book.m4b',
					size: 42,
					duration: 3600,
					isValid: true,
				},
			],
			totalDuration: 3600,
			totalSize: 42,
			validCount: 1,
			invalidCount: 0,
			selectedDecoders: [null],
		};
		context.startRemoteSourceAcquisitionMock.mockResolvedValueOnce(terminalJob);
		context.getCurrentFileListMock.mockReturnValueOnce(currentFileList);
		remoteSourceAcquireState.isOpen = true;
		render(RemoteSourceAcquireDialog);

		await screen.findByText('Mock Audible Book');
		await user.click(screen.getByRole('button', { name: /Mock Audible Book/i }));
		await user.click(screen.getByRole('button', { name: 'Acquire Selected' }));

		await screen.findByText('1 acquired title imported.');
		expect(context.handleImportedAudioPathsMock).toHaveBeenCalledWith(['/tmp/remote/book.m4b']);
		expect(context.registerRemoteSourceSupplementalAssetsMock).toHaveBeenCalledWith(
			terminalJob,
			currentFileList,
		);
		expect(context.purgeRemoteSourceSessionMock).not.toHaveBeenCalled();
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
					currentTitleId: 'B000000001',
					currentItemIndex: 1,
					totalItems: 1,
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
