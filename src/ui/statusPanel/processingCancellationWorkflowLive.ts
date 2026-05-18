import { tauriClient } from '../../lib/tauri/client';
import * as feedback from './feedback';
import {
	makeProcessingCancellationWorkflowServicesLayer,
	type ProcessingCancellationWorkflowServices,
} from './processingCancellationWorkflowServices';

export const liveProcessingCancellationWorkflowServices: ProcessingCancellationWorkflowServices = {
	cancelProcessing: (jobId?: string | null) => tauriClient.cancelProcessing(jobId),
	setCancelAllButtonPending: feedback.setCancelAllButtonPending,
	showError: feedback.showError,
	console,
};

export const ProcessingCancellationWorkflowLive = makeProcessingCancellationWorkflowServicesLayer(
	liveProcessingCancellationWorkflowServices,
);
