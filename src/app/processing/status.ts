import type { OutputSnapshot, SubmissionStatus } from '../../lib/generated/tauri';
import { concurrencyText, previewView, type ConcurrencyRead } from './render';
import { submissionLine, type SubmissionLine } from './submit';
import { IDLE_STATUS_VIEW, type StatusView } from './view';

export type StatusInputs = {
	readonly previewRun: OutputSnapshot['previewRun'];
	readonly submission: SubmissionStatus | null;
	readonly localError: string | null;
	readonly concurrency: ConcurrencyRead;
};

export function statusOf({
	previewRun,
	submission,
	localError,
	concurrency,
}: StatusInputs): StatusView {
	const view: StatusView = {
		...(previewRun ? previewView(previewRun) : IDLE_STATUS_VIEW),
		concurrencyText: concurrencyText(concurrency),
	};
	const line: SubmissionLine | null =
		localError === null ? submissionLine(submission) : { kind: 'error', text: localError };
	switch (line?.kind) {
		case undefined:
			return view;
		case 'step':
			return { ...view, stepText: `Current Step: ${line.text}` };
		case 'completed':
			return {
				...view,
				statusText: 'Completed',
				progressPercentage: 100,
				progressActive: false,
				stepText: `Current Step: ${line.text}`,
			};
		case 'error':
			return { ...view, stepText: `Error: ${line.text}`, stepTone: 'error' };
	}
}
