import type { ProcessingStatus } from './state';

export { formatEtaRemaining } from '../../lib/format/eta';

function assertNever(value: never): never {
	throw new Error(`Unhandled status stage: ${String(value)}`);
}

export function formatStatusDisplayText(stage: ProcessingStatus['stage']): string {
	switch (stage) {
		case 'idle':
			return 'Idle';
		case 'analyzing':
			return 'Analyzing';
		case 'converting':
			return 'Converting';
		case 'writing':
			return 'Writing Metadata';
		case 'completed':
			return 'Completed';
		case 'skipped':
			return 'Skipped';
		case 'cancelled':
			return 'Cancelled';
		case 'failed':
			return 'Failed';
	}
	return assertNever(stage);
}
