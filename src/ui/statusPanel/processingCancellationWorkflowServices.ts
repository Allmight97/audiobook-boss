import type { tauriClient } from '../../lib/tauri/client';
import {
	type AppLayer,
	makeWorkflowLayer,
	makeWorkflowServiceTag,
} from '../../lib/effect/appEffect';
import type * as feedback from './feedback';

export interface ProcessingCancellationWorkflowServices {
	cancelProcessing: typeof tauriClient.cancelProcessing;
	setCancelAllButtonPending: typeof feedback.setCancelAllButtonPending;
	showError: typeof feedback.showError;
	console: Pick<Console, 'error'>;
}

export type ProcessingCancellationWorkflowServicesId =
	'StatusPanel/ProcessingCancellationWorkflowServices';
export type ProcessingCancellationWorkflowLayer =
	AppLayer<ProcessingCancellationWorkflowServicesId>;

export const ProcessingCancellationWorkflowServicesTag = makeWorkflowServiceTag<
	ProcessingCancellationWorkflowServicesId,
	ProcessingCancellationWorkflowServices
>('StatusPanel/ProcessingCancellationWorkflowServices');

export function makeProcessingCancellationWorkflowServicesLayer(
	services: ProcessingCancellationWorkflowServices,
): ProcessingCancellationWorkflowLayer {
	return makeWorkflowLayer(ProcessingCancellationWorkflowServicesTag, services);
}
