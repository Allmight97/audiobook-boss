import { isCancellation, toUserMessage } from '../../lib/tauri/appError';
import type { SubmissionStatus, SubmitRefusal } from '../../types/session';
import type { ProcessingStatus } from './state';

export interface SubmissionDisplay {
	updateStatus: (status: ProcessingStatus) => void;
	setProcessingState: (active: boolean) => void;
	handleCancellation: () => void;
	resetToIdle: () => void;
	showError: (message: string) => void;
}

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
		case 'invalidPreviewLength':
			return 'Choose a preview length longer than zero seconds.';
		case 'sourceRemoved':
			return 'A downloaded source was removed after its export. Acquire it again to export it.';
		case 'closing':
			return 'ABB is closing.';
		case 'restartStale':
			return 'That restart no longer matches the latest Save or output settings. Save again to see where the title would go.';
	}
}

export function renderSubmission(
	display: SubmissionDisplay,
	status: SubmissionStatus | null,
): void {
	switch (status?.kind) {
		case 'submitted':
			display.setProcessingState(false);
			display.updateStatus({
				stage: 'completed',
				percentage: 100,
				message: 'Submitted to Work Center.',
			});
			return;
		case 'finishedBeforeRestart':
			display.setProcessingState(false);
			display.updateStatus({
				stage: 'completed',
				percentage: 100,
				message:
					status.outputs.failed > 0
						? 'The title finished before it could restart, and its tags could not be updated there. Save again to retry.'
						: 'The title finished before it could restart; its tags were updated where it is.',
			});
			return;
		case 'refused':
			display.resetToIdle();
			display.showError(refusalText(status.reason));
			return;
		case 'blocked':
			display.resetToIdle();
			display.showError(status.message);
			return;
		case 'failed':
			if (isCancellation(status.error)) {
				display.handleCancellation();
				return;
			}
			display.resetToIdle();
			display.showError(
				`Processing failed: ${toUserMessage(status.error, { fallback: 'Processing failed.' })}`,
			);
			return;
		case 'cancelled':
			display.resetToIdle();
			return;
	}
}
