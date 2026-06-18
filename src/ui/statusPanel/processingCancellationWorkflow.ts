import {
	Data,
	Effect,
	type AppLayer,
	type AppEffect,
	makeWorkflowLayer,
	makeWorkflowServiceTag,
	runAppEffect,
	workflowTryPromise,
} from '../../lib/effect/appEffect';
import { tauriClient } from '../../lib/tauri/client';
import type { ProcessingStatus } from './state';
import { setStatusPanelCancelAllPending, showError } from './viewState.svelte';

export interface ProcessingCancellationWorkflowServices {
	cancelProcessing: typeof tauriClient.cancelProcessing;
	setCancelAllButtonPending: (isPending: boolean) => void;
	showError: (message: string) => void;
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

export const liveProcessingCancellationWorkflowServices = {
	cancelProcessing: (jobId?: string | null) => tauriClient.cancelProcessing(jobId),
	setCancelAllButtonPending: setStatusPanelCancelAllPending,
	showError,
	console,
} satisfies ProcessingCancellationWorkflowServices;

export const ProcessingCancellationWorkflowLive = makeProcessingCancellationWorkflowServicesLayer(
	liveProcessingCancellationWorkflowServices,
);

export interface PreparedCancelAllRequest {
	readonly request: Promise<readonly unknown[]>;
}

export type ProcessingCancellationWorkflowAction =
	| {
			type: 'cancelAll';
			jobIds: readonly string[];
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
		if (action.jobIds.length === 0) {
			return;
		}
		if (!preparedRequest) {
			services.setCancelAllButtonPending(true);
		}
		yield* workflowPromise(
			() =>
				preparedRequest?.request ??
				Promise.all(action.jobIds.map((jobId) => services.cancelProcessing(jobId))),
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
	jobIds: readonly string[],
): PreparedCancelAllRequest {
	services.setCancelAllButtonPending(true);
	try {
		return { request: Promise.all(jobIds.map((jobId) => services.cancelProcessing(jobId))) };
	} catch (cause) {
		return { request: Promise.reject(cause) };
	}
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
	const workflowLayer = layer ?? ProcessingCancellationWorkflowLive;
	return runAppEffect(
		processingCancellationWorkflowBody(action, preparedCancelAll).pipe(
			Effect.provide(workflowLayer),
		),
	);
}
