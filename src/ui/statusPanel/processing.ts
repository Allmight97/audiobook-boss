import { tauriClient } from '../../lib/tauri/client';
import type { OutputConfig, OutputKind } from '../../types/audio';
import type { AudiobookMetadata } from '../../types/metadata';
import { getCurrentFileList, getSelectedFileIndex, setFileOrderLocked } from '../fileList';
import { getSelectedFileIndices } from '../fileList/state';
import { readOutputConfigForProcessing, updateOutputPath } from '../outputPanel';
import { getJobType, setJobControlsEnabled } from '../jobControls';
import { hasDirtyMetadataFields, readMetadataForm } from '../metadataForm';
import {
	getAllMetadataIntentPatches,
	getMetadataForFile,
	getMetadataIntentPatchForFile,
	setMetadataForFile,
} from '../metadataState';
import { stageMetadataToSelection } from '../fileList/actions';
import {
	applyMetadataIntentPatch,
	buildMetadataIntentPatchFromMetadata,
	hasActionableMetadataIntentPatch,
	type MetadataIntentPatch,
} from '../../types/metadataIntent';
import * as feedback from './feedback';
import { openGeneratedPreviewIfSingle } from './preview';
import type { ProcessingStatus } from './state';
import { isProcessingCancellationError, normalizeProcessingErrorMessage } from './errorHelpers';
import { reviewOutputPlanForProcessing } from './outputPlanReview';
import type { ProcessCommandResult, ProcessPayload } from '../../types/audio';

interface StartProcessingContext {
	updateStatus: (status: ProcessingStatus) => void;
	setProcessingState: (isProcessing: boolean) => void;
	updateArtThumbnail: () => Promise<void>;
	startProgressListener: () => Promise<void>;
	setCurrentJobType: (jobType: 'merge' | 'batch') => void;
	setBatchCompletionMessage: (message: string | null) => void;
	reconcileProcessResult?: (result: ProcessCommandResult) => void;
	handleCancellation: () => void;
	resetToIdle: () => void;
}

function summarizeBatchOutcome(result: ProcessCommandResult, filePaths: string[]): string | null {
	if (result.jobType !== 'batch') {
		return null;
	}

	const total = result.summary?.total ?? result.results.length;
	const succeeded =
		result.summary?.succeeded ??
		result.results.filter((entry) => entry.status === 'success').length;
	const skipped =
		result.summary?.skipped ?? result.results.filter((entry) => entry.status === 'skipped').length;
	const cancelled =
		result.summary?.cancelled ??
		result.results.filter((entry) => entry.status === 'cancelled').length;
	const failed =
		result.summary?.failed ?? result.results.filter((entry) => entry.status === 'failed').length;

	if (failed <= 0 && skipped <= 0 && cancelled <= 0) {
		return null;
	}

	const failedNames = Array.from(
		new Set(
			result.results
				.filter((entry) => entry.status === 'failed')
				.map((entry) => {
					if (typeof entry.inputIndex === 'number') {
						const path = filePaths[entry.inputIndex];
						if (path) {
							return path.split(/[\\/]/).pop() || path;
						}
					}
					if (entry.error != null) {
						const errorMessage = normalizeProcessingErrorMessage(entry.error, '');
						if (errorMessage.length > 0) {
							return errorMessage;
						}
					}
					if (typeof entry.message === 'string' && entry.message.length > 0) {
						return entry.message;
					}
					return 'Unknown failure';
				}),
		),
	);

	const visibleNames = failedNames.slice(0, 2);
	const moreCount = Math.max(0, failed - visibleNames.length);
	const failureSuffix =
		visibleNames.length > 0
			? ` Failed: ${visibleNames.join(', ')}${moreCount > 0 ? ` (+${moreCount} more)` : ''}`
			: '';
	const skippedSuffix = skipped > 0 ? ` Skipped: ${skipped}.` : '';
	const cancelledSuffix = cancelled > 0 ? ` Cancelled: ${cancelled}.` : '';

	if (succeeded <= 0) {
		return `No files were processed successfully.${skippedSuffix}${cancelledSuffix}${failureSuffix}`;
	}

	return `Processed ${succeeded}/${total}.${skippedSuffix}${cancelledSuffix}${failureSuffix}`;
}

export async function startProcessing(
	context: StartProcessingContext,
	options?: {
		previewSeconds?: number;
	},
): Promise<void> {
	const outputKind: OutputKind = options?.previewSeconds == null ? 'final' : 'preview';

	try {
		updateOutputPath(outputKind);
		const fileList = getCurrentFileList();
		console.log('StatusPanel: Starting processing...');
		console.log('Current file list:', fileList);
		context.setBatchCompletionMessage(null);

		// Validate inputs
		if (!fileList?.files?.length) {
			console.log('StatusPanel: No files found');
			feedback.showError('No audio files selected. Please add files to process.');
			return;
		}

		if (fileList.validCount === 0) {
			console.log('StatusPanel: No valid files found');
			feedback.showError('No valid audio files found. Please check your files and try again.');
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
			feedback.showError(`Settings validation failed: ${error}`);
			return;
		}

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

		const processPayload: ProcessPayload = {
			inputFiles: filePaths,
			outputDir: outputConfig.outputPath,
			settings: outputConfig.encoderSettings,
			externalToolchain: outputConfig.toolchainSettings,
			sampleRate: outputConfig.sampleRate,
			jobType,
			outputNaming: outputConfig.outputNaming,
		};

		if (processPayload.jobType === 'batch') {
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

		let metadataIntentPayload: Record<string, MetadataIntentPatch> | null = null;
		if (processPayload.jobType === 'merge') {
			const mergeKey = processPayload.inputFiles[0];
			const mergeIntentPatch = mergeKey ? getMetadataIntentPatchForFile(mergeKey) : undefined;
			if (
				mergeKey &&
				processPayload.inputFiles.length > 0 &&
				hasActionableMetadataIntentPatch(mergeIntentPatch)
			) {
				metadataIntentPayload = {
					[mergeKey]: mergeIntentPatch,
				};
			}
		} else {
			const storedMetadataIntent = getAllMetadataIntentPatches();
			const activeInputFiles = new Set(processPayload.inputFiles);
			const filteredMetadataIntent = Object.fromEntries(
				Object.entries(storedMetadataIntent).filter(
					([filePath, value]) =>
						activeInputFiles.has(filePath) && hasActionableMetadataIntentPatch(value),
				),
			);
			metadataIntentPayload =
				Object.keys(filteredMetadataIntent).length > 0 ? filteredMetadataIntent : null;
		}

		const reviewResult = await reviewOutputPlanForProcessing({
			payload: processPayload,
			metadataIntent: metadataIntentPayload,
			previewSeconds: options?.previewSeconds,
		});
		if (reviewResult.status === 'blocked') {
			feedback.showError(reviewResult.message);
			return;
		}
		if (reviewResult.status === 'cancelled') {
			return;
		}

		// Update UI to processing state only after output planning is approved.
		context.setProcessingState(true);
		context.updateStatus({
			stage: 'analyzing',
			percentage: 0,
			message: 'Starting processing...',
		});

		setJobControlsEnabled(false);
		setFileOrderLocked(true);

		await context.updateArtThumbnail();
		await context.startProgressListener();

		const result = await tauriClient.processAudiobookFiles({
			payload: reviewResult.payload,
			metadataIntent: metadataIntentPayload,
			previewSeconds: options?.previewSeconds,
		});

		console.log('Processing command resolved:', result);
		context.reconcileProcessResult?.(result);
		context.setBatchCompletionMessage(summarizeBatchOutcome(result, filePaths));
		await openGeneratedPreviewIfSingle(result);
	} catch (error) {
		const msg = normalizeProcessingErrorMessage(error);
		const wasCancelled =
			isProcessingCancellationError(error) || msg.toLowerCase().includes('cancelled');
		if (wasCancelled) {
			context.handleCancellation();
			return;
		}
		if (!wasCancelled) {
			console.error('Processing failed:', error);
			feedback.showError(`Processing failed: ${msg}`);
		}
		context.resetToIdle();
	} finally {
		updateOutputPath('final');
	}
}
