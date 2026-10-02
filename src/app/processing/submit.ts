import { pathBasename } from '../../lib/path/basename';
import { isCancellation, toUserMessage } from '../../lib/tauri/appError';
import type { CollisionPolicy, PlannedOutput, ProcessCommandResult } from '../../types/audio';
import type { RestartOffer, SubmissionStatus, SubmitRefusal } from '../../types/session';
import type { EngineLink } from '../engineLink';
import { openGeneratedPreviewIfSingle } from './preview';
import type { ProcessingStatus } from './state';

/** What the status panel exposes to a submission. */
export interface ProcessingWorkflowContext {
	updateStatus: (status: ProcessingStatus) => void;
	setProcessingState: (isProcessing: boolean) => void;
	updateArtThumbnail: () => Promise<void>;
	startProgressListener: () => Promise<void>;
	setBatchCompletionMessage: (message: string | null) => void;
	reconcileProcessResult?: (result: ProcessCommandResult) => void;
	handleCancellation: () => void;
	resetToIdle: () => void;
}

export type SubmitDeps = {
	readonly link: Pick<EngineLink, 'send' | 'output'>;
	/** Asks the user what to do with outputs that already exist; `null` cancels. */
	readonly reviewCollisions: (outputs: readonly PlannedOutput[]) => Promise<CollisionPolicy | null>;
	/** The paths of the valid titles, to name failures in a preview summary. */
	readonly titlePaths: () => string[];
	readonly setControlsEnabled: (enabled: boolean) => void;
	readonly showError: (message: string) => void;
};

/** Words why the engine refused a submission. */
function refusalText(reason: SubmitRefusal): string {
	switch (reason.kind) {
		case 'noTitles':
			return 'No audio files selected. Please add files to process.';
		case 'noValidTitles':
			return 'No valid audio files found. Please check your files and try again.';
		case 'noOutputDirectory':
			return 'Choose an output folder before processing.';
		case 'invalidSource':
			return 'Remove or replace invalid source files before processing.';
		case 'audioChoiceRequired':
			return 'Choose audio handling for each grouped title before processing.';
		case 'chapterReview':
		case 'draftInvalid':
			return reason.message;
		case 'noTarget':
			return 'Select a valid input file before processing metadata edits.';
		case 'saveInProgress':
			return 'Wait for the metadata save to finish before processing.';
		case 'busy':
			return 'Processing is already starting.';
		case 'sourceRemoved':
			return 'A downloaded source was removed after its export. Acquire it again to export it.';
		case 'closing':
			return 'ABB is closing.';
		case 'restartStale':
			return 'That restart no longer matches the latest Save or output settings. Save again to see where the title would go.';
	}
}

function summarizeBatchOutcome(result: ProcessCommandResult, filePaths: string[]): string | null {
	const { total, succeeded, skipped, cancelled, failed } = result.summary;
	if (failed <= 0 && skipped <= 0 && cancelled <= 0) return null;
	const failedNames = Array.from(
		new Set(
			result.results
				.filter((entry) => entry.status === 'failed')
				.map((entry) => {
					const path = filePaths[entry.inputIndex];
					if (path) return pathBasename(path, { fallback: 'path' });
					const message = entry.error ? toUserMessage(entry.error, { fallback: '' }) : '';
					return message || entry.message || 'Unknown failure';
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

/** The submission's status once the engine has answered, after any collision review. */
async function settle(
	deps: SubmitDeps,
	first: () => Promise<unknown>,
): Promise<SubmissionStatus | null> {
	await first();
	let status = deps.link.output().submission;
	while (status?.kind === 'reviewRequired') {
		const policy = await deps.reviewCollisions(status.outputs);
		await deps.link.send(
			policy ? { kind: 'chooseCollisionPolicy', policy } : { kind: 'cancelCollisionReview' },
		);
		status = deps.link.output().submission;
	}
	return status;
}

/**
 * Sends the session to the engine as an export, as a preview when
 * `previewSeconds` is given, or as one title's restart at the location a
 * Save offered, and shows how it went in the status panel.
 */
export async function runSubmission(
	context: ProcessingWorkflowContext,
	deps: SubmitDeps,
	options?: { previewSeconds?: number; restart?: RestartOffer },
): Promise<void> {
	context.setBatchCompletionMessage(null);
	const previewSeconds = options?.previewSeconds;
	deps.setControlsEnabled(false);
	try {
		let status: SubmissionStatus | null;
		if (previewSeconds != null) {
			context.setProcessingState(true);
			context.updateStatus({
				stage: 'analyzing',
				percentage: 0,
				message: 'Starting processing...',
			});
			await context.updateArtThumbnail();
			await context.startProgressListener();
			status = await settle(deps, () =>
				deps.link.send({ kind: 'preview', seconds: previewSeconds }),
			);
		} else {
			await context.updateArtThumbnail();
			const restart = options?.restart;
			status = await settle(deps, () =>
				deps.link.send(
					restart
						? { kind: 'restartTitle', titleId: restart.titleId, revision: restart.revision }
						: { kind: 'submit' },
				),
			);
		}
		await show(context, deps, status);
	} catch (cause) {
		if (isCancellation(cause)) {
			context.handleCancellation();
			return;
		}
		context.resetToIdle();
		deps.showError(`Processing failed: ${toUserMessage(cause)}`);
	} finally {
		deps.setControlsEnabled(true);
	}
}

async function show(
	context: ProcessingWorkflowContext,
	deps: SubmitDeps,
	status: SubmissionStatus | null,
): Promise<void> {
	switch (status?.kind) {
		case 'submitted':
			context.setProcessingState(false);
			context.updateStatus({
				stage: 'completed',
				percentage: 100,
				message: 'Submitted to Work Center.',
			});
			return;
		case 'finishedBeforeRestart':
			context.setProcessingState(false);
			context.updateStatus({
				stage: 'completed',
				percentage: 100,
				message:
					status.outputs.failed > 0
						? 'The title finished before it could restart, and its tags could not be updated there. Save again to retry.'
						: 'The title finished before it could restart; its tags were updated where it is.',
			});
			return;
		case 'previewFinished':
			context.reconcileProcessResult?.(status.result);
			context.setBatchCompletionMessage(summarizeBatchOutcome(status.result, deps.titlePaths()));
			await openGeneratedPreviewIfSingle(status.result);
			return;
		case 'refused':
			context.resetToIdle();
			deps.showError(refusalText(status.reason));
			return;
		case 'blocked':
			context.resetToIdle();
			deps.showError(status.message);
			return;
		case 'failed':
			if (isCancellation(status.error)) {
				context.handleCancellation();
				return;
			}
			context.resetToIdle();
			deps.showError(
				`Processing failed: ${toUserMessage(status.error, { fallback: 'Processing failed.' })}`,
			);
			return;
		default:
			// Cancelled at review, or no answer: nothing ran.
			context.resetToIdle();
	}
}
