/**
 * UI Workflow Smoke Test (docs/ubiquitous-language.md).
 *
 * Keep this as one golden-path composition proof. Owner tests cover their
 * isolated branches; this test protects the user workflow that joins them at
 * the Tauri submission boundary.
 */
import { cleanup, render, waitFor } from '@solidjs/testing-library';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createTestAppRuntime } from '../../app/runtime/harness';
import { AppRuntimeProvider } from '../../app/runtime/RuntimeProvider';
import type { FileListInfo, ProcessingPreflightPlan } from '../../types/audio';
import type { AppSettings } from '../../types/appSettings';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import type { OnlineMetadataResult } from '../../types/metadata';
import type { WorkSubmissionAccepted } from '../../types/workRuntime';
import { runtimeSettingsCapabilitiesFixture } from '../../test/fixtures/runtimeSettingsCapabilities';
import { JPEG_DATA_URL, stagedCoverView } from '../../test/fixtures/coverCapability';
import { App } from '../App';

const native = vi.hoisted(() => ({
	openFiles: vi.fn(),
	openDirectory: vi.fn(),
	discoverAudioImportPaths: vi.fn(),
	analyzeAudioFiles: vi.fn(),
	getSupportedAudioImportMetadata: vi.fn(),
	takeOpenedAudioFiles: vi.fn(),
	readAudioCoverThumbnail: vi.fn(),
	listen: vi.fn(),
	readAudioMetadata: vi.fn(),
	validateMetadataIntentPatch: vi.fn(),
	saveMetadataBatch: vi.fn(),
	openFile: vi.fn(),
	loadCoverArtFile: vi.fn(),
	loadCoverArtFromUrl: vi.fn(),
	previewCoverArtFromUrl: vi.fn(),
	searchOnlineMetadata: vi.fn(),
	getAppSettings: vi.fn(),
	updateAppSettings: vi.fn(),
	resetAppSettings: vi.fn(),
	getMaxConcurrentJobs: vi.fn(),
	setMaxConcurrentJobs: vi.fn(),
	getRuntimeSettingsCapabilities: vi.fn(),
	previewOutputPath: vi.fn(),
	preflightProcessingPlan: vi.fn(),
	processAudiobookFiles: vi.fn(),
	submitProcessingOperation: vi.fn(),
	listWorkOperations: vi.fn(),
	cancelWorkOperation: vi.fn(),
	openPath: vi.fn(),
	purgeRemoteSourceSession: vi.fn(),
}));

vi.mock('../../lib/tauri/client', () => ({ tauriClient: native }));

const INPUT_PATH = '/library/Dune.m4b';
const OUTPUT_DIRECTORY = '/exports/audiobooks';
const COVER_HANDLE = 'cover-1';
const COVER_VIEW = stagedCoverView(COVER_HANDLE, JPEG_DATA_URL);

function fileList(): FileListInfo {
	return {
		files: [
			{
				inputId: 'input-dune',
				path: INPUT_PATH,
				size: 1024,
				duration: 3600,
				format: 'm4b',
				bitrate: 64,
				sampleRate: 44100,
				channels: 1,
				codecLabel: 'AAC',
				selectedDecoder: 'FFmpeg',
				tagTitle: 'Dune (old tags)',
				tagArtist: 'Old Author',
				isValid: true,
			},
		],
		selectedDecoders: [null],
		totalDuration: 3600,
		totalSize: 1024,
		validCount: 1,
		invalidCount: 0,
	};
}

function lookupResult(): OnlineMetadataResult {
	return {
		source: 'audnexus',
		sourceId: 'dune-1965',
		title: 'Dune',
		authors: ['Frank Herbert'],
		narrators: ['George Guidall'],
		series: 'Dune',
		seriesPart: '1',
		subseries: 'Dune Saga',
		subseriesPart: '1',
		description: 'The desert planet Arrakis holds the spice.',
		publishedDate: '1965-08',
		durationSeconds: 3600,
		coverUrl: 'https://covers.example.test/dune.jpg',
		audibleOnly: false,
	};
}

function appSettings(): AppSettings {
	return {
		maxConcurrentJobs: { mode: 'auto' },
		encoderDefaults: {
			settings: {
				encoderType: 'auto',
				bitrateKbps: 64,
				bitrateMode: { mode: 'vbr', value: 3 },
				channels: 'auto',
				afterburner: true,
			},
			sampleRate: 'auto',
		},
		outputDefaults: {
			outputNaming: { preset: 'absDefault', includeYear: false },
		},
		toolchain: {},
		startupBehavior: 'rememberLastState',
	};
}

function approvedPlan(): ProcessingPreflightPlan {
	return {
		jobType: 'batch',
		previewSeconds: undefined,
		collisionPolicy: 'fail',
		planSignature: 'smoke-preflight',
		outputs: [
			{
				inputIndex: 0,
				inputPath: INPUT_PATH,
				kind: 'final',
				requestedPath: `${OUTPUT_DIRECTORY}/Frank Herbert/Dune (1965)/Dune.m4b`,
				resolvedPath: `${OUTPUT_DIRECTORY}/Frank Herbert/Dune (1965)/Dune.m4b`,
				action: 'write',
			},
		],
	};
}

function acceptedSubmission(): WorkSubmissionAccepted {
	return {
		operationId: 'operation-smoke',
		snapshot: {
			operationId: 'operation-smoke',
			sequence: 1,
			kind: 'processingBatch',
			status: 'accepted',
			title: 'Batch encode (1 file)',
			createdAtMs: 1,
			cancellable: true,
			cancelRequested: false,
			lanes: ['analysis', 'encodeCpu', 'outputCommit'],
			sourceInputIds: ['input-dune'],
			progress: {
				stage: 'pending',
				percentage: 0,
				message: 'Accepted.',
				totalItems: 1,
			},
			children: [],
			warnings: [],
			errors: [],
			logTail: [],
		},
	};
}

describe('UI Workflow Smoke Test', () => {
	afterEach(() => cleanup());

	it('submits lookup metadata, cover art, output, and encoder intent through the Tauri boundary', async () => {
		const settings = appSettings();
		native.openFiles.mockResolvedValue([INPUT_PATH]);
		native.openDirectory.mockResolvedValue(OUTPUT_DIRECTORY);
		native.discoverAudioImportPaths.mockImplementation(async (paths) => [...paths]);
		native.analyzeAudioFiles.mockResolvedValue(fileList());
		native.getSupportedAudioImportMetadata.mockResolvedValue({
			formats: [{ extension: 'm4b', label: 'M4B' }],
			extensions: ['m4b'],
			formatsText: 'M4B',
			supportText: 'Supports M4B audio files',
		});
		native.takeOpenedAudioFiles.mockResolvedValue([]);
		native.readAudioCoverThumbnail.mockResolvedValue(null);
		native.listen.mockResolvedValue(() => undefined);
		native.readAudioMetadata.mockResolvedValue({
			title: 'Dune (old tags)',
			artist: 'Old Author',
		});
		native.validateMetadataIntentPatch.mockImplementation(
			async (metadataPatch: MetadataIntentPatch) => ({
				isValid: true,
				metadataPatch,
				fieldErrors: [],
			}),
		);
		native.loadCoverArtFromUrl.mockResolvedValue(COVER_VIEW);
		native.previewCoverArtFromUrl.mockResolvedValue({
			handleId: null,
			dataUrl: JPEG_DATA_URL,
		});
		native.searchOnlineMetadata.mockResolvedValue({
			results: [lookupResult()],
			diagnostics: [],
		});
		native.getAppSettings.mockResolvedValue(settings);
		native.updateAppSettings.mockResolvedValue(settings);
		native.resetAppSettings.mockResolvedValue(settings);
		native.getMaxConcurrentJobs.mockResolvedValue(4);
		native.setMaxConcurrentJobs.mockResolvedValue(4);
		native.getRuntimeSettingsCapabilities.mockResolvedValue(runtimeSettingsCapabilitiesFixture());
		native.previewOutputPath.mockResolvedValue(
			`${OUTPUT_DIRECTORY}/Frank Herbert/Dune (1965)/Dune.m4b`,
		);
		native.preflightProcessingPlan.mockResolvedValue(approvedPlan());
		native.submitProcessingOperation.mockResolvedValue(acceptedSubmission());
		native.listWorkOperations.mockResolvedValue({ operations: [] });

		const runtime = createTestAppRuntime();
		const user = userEvent.setup();
		render(() => (
			<AppRuntimeProvider runtime={runtime}>
				<App />
			</AppRuntimeProvider>
		));

		try {
			await waitFor(() => {
				expect(document.getElementById('adv-encoder')).not.toBeDisabled();
			});
			await user.click(document.querySelector('[aria-label="Add audio files"]') as HTMLElement);
			await waitFor(() => {
				expect(document.getElementById('meta-title')).toHaveValue('Dune (old tags)');
			});

			await user.click(document.getElementById('metadata-lookup-btn') as HTMLElement);
			const useMetadata = await waitFor(() => {
				const button = document.querySelector<HTMLButtonElement>(
					"#metadata-lookup-results button[data-index='0']",
				);
				if (!button) throw new Error('Lookup result did not render');
				return button;
			});
			await user.click(document.getElementById('metadata-lookup-cover-toggle') as HTMLElement);
			await user.click(useMetadata);
			await waitFor(() => {
				expect(document.getElementById('meta-title')).toHaveValue('Dune');
				expect(document.getElementById('cover-art-img')).not.toHaveClass('hidden');
			});

			await user.selectOptions(
				document.getElementById('adv-encoder') as HTMLSelectElement,
				'native_aac',
			);
			await user.selectOptions(
				document.getElementById('output-bitrate') as HTMLSelectElement,
				'96',
			);
			await user.selectOptions(
				document.getElementById('output-samplerate') as HTMLSelectElement,
				'44100',
			);
			await user.selectOptions(
				document.getElementById('output-channels') as HTMLSelectElement,
				'mono',
			);
			await user.click(document.getElementById('output-dir-browse') as HTMLElement);
			await user.click(document.getElementById('output-abs-include-year') as HTMLElement);
			await waitFor(() => {
				expect(document.getElementById('output-dir-text')).toHaveTextContent(OUTPUT_DIRECTORY);
			});

			await user.click(document.getElementById('process-button') as HTMLElement);
			await waitFor(() => {
				expect(native.submitProcessingOperation).toHaveBeenCalledTimes(1);
			});

			expect(native.submitProcessingOperation).toHaveBeenCalledWith({
				payload: {
					inputFiles: [INPUT_PATH],
					inputIds: ['input-dune'],
					outputDir: OUTPUT_DIRECTORY,
					settings: {
						encoderType: 'native_aac',
						bitrateKbps: 96,
						bitrateMode: { mode: 'cbr' },
						channels: 'mono',
						afterburner: false,
					},
					sampleRate: { explicit: 44100 },
					jobType: 'batch',
					outputNaming: {
						preset: 'absDefault',
						includeYear: true,
						customTemplate: undefined,
					},
					supplementalAssetsByInputId: undefined,
					collisionPolicy: 'fail',
					preflightSignature: 'smoke-preflight',
				},
				metadataIntent: {
					[INPUT_PATH]: {
						title: { op: 'set', value: 'Dune' },
						artist: { op: 'set', value: 'Frank Herbert' },
						album: { op: 'set', value: 'Dune' },
						composer: { op: 'set', value: 'George Guidall' },
						date: { op: 'set', value: '1965-08' },
						description: {
							op: 'set',
							value: 'The desert planet Arrakis holds the spice.',
						},
						series: { op: 'set', value: 'Dune' },
						series_part: { op: 'set', value: '1' },
						subseries: { op: 'set', value: 'Dune Saga' },
						subseries_part: { op: 'set', value: '1' },
						cover_art: { op: 'set', value: COVER_HANDLE },
					},
				},
				previewSeconds: undefined,
			});
		} finally {
			runtime.dispose();
		}
	});
});
