import { pathBasename } from '../../lib/path/basename';
import type { MetadataSaveBatchResult } from '../../types/metadata';
import type { StatusPanelCompletionFeedback } from './domain/stateMachine';

export function buildMetadataSaveCompletionFeedback(
	result: MetadataSaveBatchResult,
): StatusPanelCompletionFeedback {
	const total = result.summary.total;
	const succeeded = result.summary.succeeded;
	const failedEntries = result.results.filter((entry) => entry.status === 'failed');
	const cancelled = result.summary.cancelled;

	if (failedEntries.length > 0) {
		const failedNames = failedEntries.map((entry) => filenameFromPath(entry.filePath)).join(', ');
		const cancelledSuffix = cancelled > 0 ? ` Cancelled: ${cancelled}.` : '';
		return {
			kind: 'error',
			message: `Saved ${succeeded}/${total}. Failed: ${failedNames}.${cancelledSuffix}`,
		};
	}

	if (cancelled > 0) {
		return {
			kind: 'info',
			message:
				succeeded > 0
					? `Saved ${succeeded}/${total}. Cancelled: ${cancelled}.`
					: 'Metadata save cancelled.',
		};
	}

	return {
		kind: 'success',
		message: succeeded > 1 ? `Metadata saved (${succeeded} files)!` : 'Metadata saved!',
	};
}

function filenameFromPath(filePath: string): string {
	return pathBasename(filePath);
}
