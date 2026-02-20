import { tauriClient } from '../../lib/tauri/client';
import type { OutputConfig } from '../../types/audio';
import type { AudiobookMetadata } from '../../types/metadata';
import { getCurrentFileList, getSelectedFileIndex, setFileOrderLocked } from '../fileList';
import { getSelectedFileIndices } from '../fileList/state';
import { readOutputConfigForProcessing } from '../outputPanel';
import { getJobType, setJobControlsEnabled } from '../jobControls';
import { hasDirtyMetadataFields, readMetadataForm } from '../metadataForm';
import {
	getAllMetadata,
	getMetadataForFile,
	getMetadataIntentPatchForFile,
	setMetadataForFile,
} from '../metadataState';
import { stageMetadataToSelection } from '../fileList/actions';
import {
	applyMetadataIntentPatch,
	buildMetadataIntentPatchFromMetadata,
	hasActionableMetadataIntentPatch,
} from '../../types/metadataIntent';
import * as dom from './dom';
import type { ProcessingStatus } from './state';
import { normalizeProcessingErrorMessage } from './errorHelpers';

interface StartProcessingContext {
	updateStatus: (status: ProcessingStatus) => void;
	setProcessingState: (isProcessing: boolean) => void;
	updateArtThumbnail: () => Promise<void>;
	startProgressListener: () => Promise<void>;
	setCurrentJobType: (jobType: 'merge' | 'batch') => void;
	resetToIdle: () => void;
}

export async function startProcessing(
	context: StartProcessingContext,
	options?: {
		previewSeconds?: number;
	},
): Promise<void> {
	try {
		const fileList = getCurrentFileList();
		console.log('StatusPanel: Starting processing...');
		console.log('Current file list:', fileList);

		// Validate inputs
		if (!fileList || !fileList.files || fileList.files.length === 0) {
			console.log('StatusPanel: No files found');
			dom.showError('No audio files selected. Please add files to process.');
			return;
		}

		if (fileList.validCount === 0) {
			console.log('StatusPanel: No valid files found');
			dom.showError('No valid audio files found. Please check your files and try again.');
			return;
		}

		console.log('StatusPanel: Files validated, getting output configuration...');

		// Get output configuration
		let outputConfig: OutputConfig;
		try {
			outputConfig = readOutputConfigForProcessing();
			console.log('StatusPanel: Output configuration retrieved:', outputConfig);
		} catch (error) {
			console.log('StatusPanel: Settings validation failed:', error);
			dom.showError(`Settings validation failed: ${error}`);
			return;
		}

		// Update UI to processing state
		context.setProcessingState(true);
		context.updateStatus({
			stage: 'analyzing',
			percentage: 0,
			message: 'Starting processing...',
		});

		// Disable job controls
		setJobControlsEnabled(false);
		setFileOrderLocked(true);

		// Update art thumbnail with current file's cover art
		await context.updateArtThumbnail();

		// Start listening for progress events
		await context.startProgressListener();

		// Get file paths for processing
		const filePaths = fileList.files.filter((file) => file.isValid).map((file) => file.path);

		const selectionCount = getSelectedFileIndices().size;
		if (selectionCount > 1) {
			await stageMetadataToSelection({ showStatus: false });
		}
		let currentMetadata: Partial<AudiobookMetadata> = {};
		if (selectionCount <= 1) {
			if (hasDirtyMetadataFields()) {
				const selectedFileIndex = getSelectedFileIndex();
				const formMetadata = readMetadataForm({ mode: 'single' });
				const intentPatch = buildMetadataIntentPatchFromMetadata(formMetadata);
				const activeFile =
					selectedFileIndex >= 0
						? fileList.files[selectedFileIndex]
						: fileList.files.find((file) => file.isValid);
				if (activeFile?.isValid && hasActionableMetadataIntentPatch(intentPatch)) {
					const existing = getMetadataForFile(activeFile.path) ?? {};
					currentMetadata = applyMetadataIntentPatch(existing, intentPatch);
					setMetadataForFile(activeFile.path, currentMetadata, {
						markPending: true,
						intentPatch,
					});
				} else {
					currentMetadata = formMetadata;
				}
			}
		}

		const jobType = getJobType();
		context.setCurrentJobType(jobType);

		const v2Payload = {
			inputFiles: filePaths,
			outputDir: outputConfig.outputPath,
			settings: outputConfig.encoderSettings,
			sampleRate: outputConfig.sampleRate,
			jobType,
			outputNaming: outputConfig.outputNaming,
		};

		if (v2Payload.jobType === 'batch') {
			const missingMetadata = filePaths.filter((filePath) => !getMetadataForFile(filePath));
			if (missingMetadata.length > 0) {
				await Promise.all(
					missingMetadata.map(async (filePath) => {
						try {
							const metadata = await tauriClient.readAudioMetadata(filePath);
							setMetadataForFile(filePath, metadata);
						} catch (error) {
							console.warn('Failed to load metadata for batch file:', filePath, error);
						}
					}),
				);
			}
		}

		let metadataPayload: Record<string, Partial<AudiobookMetadata>> | null = null;
		if (v2Payload.jobType === 'merge') {
			const mergeKey = v2Payload.inputFiles[0];
			const mergeIntentPatch = mergeKey ? getMetadataIntentPatchForFile(mergeKey) : undefined;
			if (
				mergeKey &&
				v2Payload.inputFiles.length > 0 &&
				hasActionableMetadataIntentPatch(mergeIntentPatch)
			) {
				const mergeMetadata = getMetadataForFile(mergeKey) ?? currentMetadata;
				metadataPayload = {
					[mergeKey]: mergeMetadata,
				};
			}
		} else {
			const storedMetadata = getAllMetadata();
			const filteredMetadata = Object.fromEntries(
				Object.entries(storedMetadata).filter(([, value]) => Object.keys(value).length > 0),
			);
			metadataPayload = Object.keys(filteredMetadata).length > 0 ? filteredMetadata : null;
		}

		const result = await tauriClient.processAudiobookFilesV2({
			payload: v2Payload,
			metadata: metadataPayload,
			previewSeconds: options?.previewSeconds,
		});

		console.log('Processing completed successfully:', result);
		if (result?.previewFilePath) {
			const seconds =
				typeof result.previewActualSeconds === 'number'
					? result.previewActualSeconds.toFixed(3)
					: '≈30';
			console.log(`Preview file created at: ${result.previewFilePath} (${seconds}s)`);
			try {
				await tauriClient.openExternal(result.previewFilePath);
			} catch (error) {
				console.warn('Failed to open preview file automatically:', error);
			}
		}
		if (options?.previewSeconds) {
			// Optionally handle showing/opening preview file via result once backend returns a path shape
			// Placeholder: UI messaging handled by progress events for now
		}
	} catch (error) {
		const msg = normalizeProcessingErrorMessage(error);
		if (!msg.toLowerCase().includes('cancelled')) {
			console.error('Processing failed:', error);
			dom.showError(`Processing failed: ${msg}`);
		}
		context.resetToIdle();
	}
}
