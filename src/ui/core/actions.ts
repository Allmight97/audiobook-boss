import { triggerProcessFromStatusPanel } from '../statusPanel';
import { enterMetadataSaveWorkflow, runMetadataSaveWorkflow } from './metadataSaveWorkflow';
import { liveMetadataSaveWorkflowEntryServices } from './metadataSaveWorkflowEntryLive';

export async function saveMetadataFromUI(): Promise<void> {
	const preparedEntry = enterMetadataSaveWorkflow(liveMetadataSaveWorkflowEntryServices);
	if (!preparedEntry) {
		return;
	}

	try {
		const { MetadataSaveWorkflowLive } = await import('./metadataSaveWorkflowLive');
		await runMetadataSaveWorkflow(MetadataSaveWorkflowLive, preparedEntry);
	} catch (error) {
		liveMetadataSaveWorkflowEntryServices.setMetadataSaveInProgress(false);
		throw error;
	}
}

export function startPreviewAudio(duration: number): void {
	triggerProcessFromStatusPanel({ previewSeconds: duration });
}
