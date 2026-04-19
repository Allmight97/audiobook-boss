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
import * as dom from './dom';
import type { ProcessingStatus } from './state';
import { isProcessingCancellationError, normalizeProcessingErrorMessage } from './errorHelpers';
import { openCollisionDialog } from '../collisionDialog';
import type {
	CollisionPolicy,
	ProcessCommandJobResult,
	ProcessCommandResult,
	ProcessPayload,
	ProcessingPreflightPlan,
} from '../../types/audio';

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

function isSuccessfulResultEntry(entry: ProcessCommandJobResult): boolean {
	return entry.status === 'success';
}

function extractSuccessfulPreviewPaths(result: ProcessCommandResult): string[] {
	return result.results
		.filter(
			(entry) => typeof entry.previewFilePath === 'string' && entry.previewFilePath.length > 0,
		)
		.filter(isSuccessfulResultEntry)
		.map((entry) => entry.previewFilePath as string);
}

function formatFilenameForDisplay(path: string): string {
	const segments = path.split(/[\\/]/);
	return segments[segments.length - 1] || path;
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
	const failed =
		result.summary?.failed ?? result.results.filter((entry) => entry.status === 'failed').length;

	if (failed <= 0 && skipped <= 0) {
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
							return formatFilenameForDisplay(path);
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

	if (succeeded <= 0) {
		return `No files were processed successfully.${skippedSuffix}${failureSuffix}`;
	}

	return `Processed ${succeeded}/${total}.${skippedSuffix}${failureSuffix}`;
}

function getHardBlockingCollisionMessage(plan: ProcessingPreflightPlan): string | null {
	const blocked = plan.outputs.find(
		(output) =>
			output.collision?.kind === 'source_destination_overlap' ||
			output.collision?.kind === 'canonical_path_overlap',
	);
	if (!blocked) {
		return null;
	}
	return (
		blocked.collision?.detail ??
		`Output path '${blocked.requestedPath}' targets an input source file. Choose a different destination.`
	);
}

async function reviewOutputPlanBeforeProcessing(
	processPayload: ProcessPayload,
	metadataIntentPayload: Record<string, MetadataIntentPatch> | null,
	previewSeconds?: number,
): Promise<ProcessPayload | null> {
	const initialPlan = await tauriClient.preflightProcessingPlan({
		payload: processPayload,
		metadataIntent: metadataIntentPayload,
		previewSeconds,
	});
	const hardBlockMessage = getHardBlockingCollisionMessage(initialPlan);
	if (hardBlockMessage) {
		dom.showError(hardBlockMessage);
		return null;
	}

	const needsReview = initialPlan.outputs.some((output) => output.action === 'review_required');
	if (!needsReview) {
		return {
			...processPayload,
			collisionPolicy: initialPlan.collisionPolicy,
			preflightSignature: initialPlan.planSignature,
		};
	}

	const selectedPolicy = await openCollisionDialog(initialPlan);
	if (!selectedPolicy) {
		return null;
	}

	const reviewedPayload: ProcessPayload = {
		...processPayload,
		collisionPolicy: selectedPolicy as CollisionPolicy,
	};
	const reviewedPlan = await tauriClient.preflightProcessingPlan({
		payload: reviewedPayload,
		metadataIntent: metadataIntentPayload,
		previewSeconds,
	});
	const reviewedHardBlock = getHardBlockingCollisionMessage(reviewedPlan);
	if (reviewedHardBlock) {
		dom.showError(reviewedHardBlock);
		return null;
	}

	return {
		...reviewedPayload,
		preflightSignature: reviewedPlan.planSignature,
	};
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

		const reviewedPayload = await reviewOutputPlanBeforeProcessing(
			processPayload,
			metadataIntentPayload,
			options?.previewSeconds,
		);
		if (!reviewedPayload) {
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
			payload: reviewedPayload,
			metadataIntent: metadataIntentPayload,
			previewSeconds: options?.previewSeconds,
		});

		console.log('Processing command resolved:', result);
		context.reconcileProcessResult?.(result);
		context.setBatchCompletionMessage(summarizeBatchOutcome(result, filePaths));

		const previewPaths = extractSuccessfulPreviewPaths(result);
		if (previewPaths.length === 1) {
			const successfulPreview = result.results.find(
				(entry) => entry.previewFilePath === previewPaths[0] && isSuccessfulResultEntry(entry),
			);
			const seconds =
				typeof successfulPreview?.previewActualSeconds === 'number'
					? successfulPreview.previewActualSeconds.toFixed(3)
					: '≈30';
			console.log(`Preview file created at: ${previewPaths[0]} (${seconds}s)`);
			try {
				await tauriClient.openExternal(previewPaths[0]);
			} catch (error) {
				console.warn('Failed to open preview file automatically:', error);
			}
		}
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
			dom.showError(`Processing failed: ${msg}`);
		}
		context.resetToIdle();
	} finally {
		updateOutputPath('final');
	}
}
