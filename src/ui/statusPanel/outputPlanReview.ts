import {
	makeOutputPlanWorkflowServicesLayer,
	runOutputPlanReviewWorkflow,
	type OutputPlanReviewRequest,
	type OutputPlanReviewResult,
	type OutputPlanWorkflowServices,
} from '../outputPanel/outputPlanWorkflow';

interface OutputPlanReviewDeps {
	preflightProcessingPlan: OutputPlanWorkflowServices['preflightProcessingPlan'];
	openCollisionDialog: OutputPlanWorkflowServices['openCollisionDialog'];
}

export type { OutputPlanReviewRequest, OutputPlanReviewResult };

export async function reviewOutputPlanForProcessing(
	request: OutputPlanReviewRequest,
	deps: Partial<OutputPlanReviewDeps> = {},
): Promise<OutputPlanReviewResult> {
	if (!deps.preflightProcessingPlan && !deps.openCollisionDialog) {
		return runOutputPlanReviewWorkflow(request);
	}

	const live = await import('../outputPanel/outputPlanWorkflowLive');
	return runOutputPlanReviewWorkflow(
		request,
		makeOutputPlanWorkflowServicesLayer({
			...live.liveOutputPlanWorkflowServices,
			...deps,
		}),
	);
}
