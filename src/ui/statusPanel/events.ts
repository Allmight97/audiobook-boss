import { tauriClient } from '../../lib/tauri/client';
import { EVENTS } from '../../types/events';
import type { ProcessingProgressEvent, ProcessingQueueEvent } from '../../types/events';

export async function listenForProgressEvents(
	onProgress: (event: ProcessingProgressEvent) => void,
): Promise<() => void> {
	return tauriClient.listen(EVENTS.PROGRESS, (event) => {
		onProgress(event.payload);
	});
}

export async function listenForQueueEvents(
	onQueue: (event: ProcessingQueueEvent) => void,
): Promise<() => void> {
	return tauriClient.listen(EVENTS.QUEUE, (event) => {
		onQueue(event.payload);
	});
}
