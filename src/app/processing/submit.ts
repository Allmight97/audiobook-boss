import { toUserMessage } from '../../lib/tauri/appError';
import type { SubmissionStatus, SubmitRefusal } from '../../lib/generated/tauri';

export type SubmissionLine = {
	readonly kind: 'step' | 'completed' | 'error';
	readonly text: string;
};

export function processingFailure(error: unknown): string {
	const message = toUserMessage(error, { fallback: '' });
	return message ? `Processing failed: ${message}` : 'Processing failed.';
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
		case 'missingTitle':
			return `${reason.label} has no title. Select it and type a title before processing.`;
		case 'sourceRemoved':
			return 'A downloaded source was removed after its export. Acquire it again to export it.';
		case 'closing':
			return 'ABB is closing.';
		case 'restartStale':
			return 'That restart no longer matches the latest Save or output settings. Save again to see where the title would go.';
	}
}

function submittedText(leftOut: number): string {
	if (leftOut === 0) return 'Submitted to Work Center.';
	const files = leftOut === 1 ? '1 invalid file was' : `${leftOut} invalid files were`;
	return `Submitted to Work Center. ${files} left out.`;
}

export function submissionLine(status: SubmissionStatus | null): SubmissionLine | null {
	switch (status?.kind) {
		case undefined:
		case 'previewing':
		case 'previewFinished':
		case 'reviewRequired':
		case 'preparing':
		case 'cancelled':
			return null;
		case 'submitted':
			return { kind: 'step', text: submittedText(status.leftOut) };
		case 'finishedBeforeRestart':
			return {
				kind: 'completed',
				text:
					status.outputs.failed > 0
						? 'The title finished before it could restart, and its tags could not be updated there. Save again to retry.'
						: 'The title finished before it could restart; its tags were updated where it is.',
			};
		case 'refused':
			return { kind: 'error', text: refusalText(status.reason) };
		case 'blocked':
			return { kind: 'error', text: status.message };
		case 'failed':
			return { kind: 'error', text: processingFailure(status.error) };
	}
}
