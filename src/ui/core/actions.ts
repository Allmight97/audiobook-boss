import { triggerProcessFromStatusPanel } from '../statusPanel';
import {
	enterMetadataSaveWorkflow,
	liveMetadataSaveWorkflowEntryServices,
	MetadataSaveWorkflowLive,
	runMetadataSaveWorkflow,
} from './metadataSaveWorkflow';

export async function saveMetadataFromUI(): Promise<void> {
	const preparedEntry = enterMetadataSaveWorkflow(liveMetadataSaveWorkflowEntryServices);
	if (!preparedEntry) {
		return;
	}

	try {
		await runMetadataSaveWorkflow(MetadataSaveWorkflowLive, preparedEntry);
	} catch (error) {
		liveMetadataSaveWorkflowEntryServices.setMetadataSaveInProgress(false);
		throw error;
	}
}

export function startPreviewAudio(duration: number): void {
	triggerProcessFromStatusPanel({ previewSeconds: duration });
}
