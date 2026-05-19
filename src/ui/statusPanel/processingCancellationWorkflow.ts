import {
	Data,
	Effect,
	type AppEffect,
	runAppEffect,
	workflowTryPromise,
} from '../../lib/effect/appEffect';
import type { ProcessingStatus } from './state';
import {
	ProcessingCancellationWorkflowServicesTag,
	type ProcessingCancellationWorkflowLayer,
	type ProcessingCancellationWorkflowServices,
	type ProcessingCancellationWorkflowServicesId,
} from './processingCancellationWorkflowServices';

export {
	ProcessingCancellationWorkflowServicesTag,
	makeProcessingCancellationWorkflowServicesLayer,
	type ProcessingCancellationWorkflowLayer,
	type ProcessingCancellationWorkflowServices,
	type ProcessingCancellationWorkflowServicesId,
} from './processingCancellationWorkflowServices';

export interface PreparedCancelAllRequest {
	readonly request: ReturnType<ProcessingCancellationWorkflowServices['cancelProcessing']>;
}

export type ProcessingCancellationWorkflowAction =
	| {
			type: 'cancelAll';
			getCurrentStatus: () => ProcessingStatus;
			updateStatus: (status: ProcessingStatus) => void;
	  }
	| { type: 'cancelJob'; jobId: string };

export class ProcessingCancellationWorkflowFailed extends Data.TaggedError(
	'ProcessingCancellationWorkflowFailed',
)<{
	readonly message: string;
	readonly cause: unknown;
}> {}

function workflowFailure(message: string, cause: unknown): ProcessingCancellationWorkflowFailed {
	return new ProcessingCancellationWorkflowFailed({ message, cause });
}

function workflowPromise<A>(
	evaluate: () => PromiseLike<A>,
	message: string,
): AppEffect<A, ProcessingCancellationWorkflowFailed> {
	return workflowTryPromise(evaluate, message, workflowFailure);
}

function cancellationRequestedStatus(status: ProcessingStatus): ProcessingStatus {
	if (status.stage === 'idle') {
		return {
			stage: 'idle',
			percentage: 0,
			message: 'Cancellation requested…',
		};
	}

	return {
		...status,
		message: 'Cancellation requested…',
	};
}

function cancelAll(
	action: Extract<ProcessingCancellationWorkflowAction, { type: 'cancelAll' }>,
	preparedRequest?: PreparedCancelAllRequest,
): AppEffect<void, never, ProcessingCancellationWorkflowServicesId> {
	return Effect.gen(function* () {
		const services = yield* ProcessingCancellationWorkflowServicesTag;
		if (!preparedRequest) {
			services.setCancelAllButtonPending(true);
		}
		yield* workflowPromise(
			() => preparedRequest?.request ?? services.cancelProcessing(),
			'Failed to cancel processing.',
		).pipe(
			Effect.tap(() =>
				Effect.sync(() => {
					action.updateStatus(cancellationRequestedStatus(action.getCurrentStatus()));
				}),
			),
			Effect.catchAll((error) =>
				Effect.sync(() => {
					services.console.error('Failed to cancel processing:', error.cause);
					services.showError('Failed to cancel processing. Please try again.');
				}),
			),
			Effect.ensuring(Effect.sync(() => services.setCancelAllButtonPending(false))),
		);
	});
}

function cancelJob(
	action: Extract<ProcessingCancellationWorkflowAction, { type: 'cancelJob' }>,
): AppEffect<void, never, ProcessingCancellationWorkflowServicesId> {
	return Effect.gen(function* () {
		const services = yield* ProcessingCancellationWorkflowServicesTag;
		yield* workflowPromise(
			() => services.cancelProcessing(action.jobId),
			`Failed to cancel job ${action.jobId}.`,
		).pipe(
			Effect.catchAll((error) =>
				Effect.sync(() => {
					services.console.error(`Failed to cancel job ${action.jobId}:`, error.cause);
					services.showError(`Failed to cancel job ${action.jobId}`);
				}),
			),
		);
	});
}

function processingCancellationWorkflowBody(
	action: ProcessingCancellationWorkflowAction,
	preparedCancelAll?: PreparedCancelAllRequest,
): AppEffect<void, never, ProcessingCancellationWorkflowServicesId> {
	switch (action.type) {
		case 'cancelAll':
			return cancelAll(action, preparedCancelAll);
		case 'cancelJob':
			return cancelJob(action);
	}
}

export function enterCancelAllCancellationWorkflow(
	services: ProcessingCancellationWorkflowServices,
): PreparedCancelAllRequest {
	services.setCancelAllButtonPending(true);
	try {
		return { request: services.cancelProcessing() };
	} catch (cause) {
		return { request: Promise.reject(cause) as ReturnType<typeof services.cancelProcessing> };
	}
}

async function defaultProcessingCancellationWorkflowLayer(): Promise<ProcessingCancellationWorkflowLayer> {
	const live = await import('./processingCancellationWorkflowLive');
	return live.ProcessingCancellationWorkflowLive;
}

export function processingCancellationWorkflowExecution(
	action: ProcessingCancellationWorkflowAction,
): AppEffect<void, never, ProcessingCancellationWorkflowServicesId> {
	return processingCancellationWorkflowBody(action);
}

export async function runProcessingCancellationWorkflow(
	action: ProcessingCancellationWorkflowAction,
	layer?: ProcessingCancellationWorkflowLayer,
	preparedCancelAll?: PreparedCancelAllRequest,
): Promise<void> {
	const workflowLayer = layer ?? (await defaultProcessingCancellationWorkflowLayer());
	return runAppEffect(
		processingCancellationWorkflowBody(action, preparedCancelAll).pipe(
			Effect.provide(workflowLayer),
		),
	);
}
