import { STAGES } from '../../../types/events';
import type { ProcessingProgressEvent } from '../../../types/events';

const DEFAULT_PROGRESS_THROTTLE_MS = 1000;

export function isTerminalProgressEvent(event: ProcessingProgressEvent): boolean {
	return (
		event.stage === STAGES.completed ||
		event.stage === STAGES.failed ||
		event.stage === STAGES.cancelled
	);
}

export function shouldThrottleProgressUpdate(
	now: number,
	lastRender: number,
	isTerminal: boolean,
	throttleMs: number = DEFAULT_PROGRESS_THROTTLE_MS,
): boolean {
	return !isTerminal && now - lastRender < throttleMs;
}
