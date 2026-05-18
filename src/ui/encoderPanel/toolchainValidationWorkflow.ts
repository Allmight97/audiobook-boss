import { Data, Effect, type AppEffect, runAppEffect } from '../../lib/effect/appEffect';
import {
	ToolchainValidationWorkflowServicesTag,
	type ToolchainHydrationMode,
	type ToolchainValidationWorkflowAction,
	type ToolchainValidationWorkflowLayer,
	type ToolchainValidationWorkflowServicesId,
} from './toolchainValidationWorkflowServices';

export {
	ToolchainValidationWorkflowServicesTag,
	makeToolchainValidationWorkflowServicesLayer,
	type ToolchainHydrationMode,
	type ToolchainValidationWorkflowAction,
	type ToolchainValidationWorkflowLayer,
	type ToolchainValidationWorkflowServices,
	type ToolchainValidationWorkflowServicesId,
} from './toolchainValidationWorkflowServices';

export class ToolchainValidationWorkflowFailed extends Data.TaggedError(
	'ToolchainValidationWorkflowFailed',
)<{
	readonly message: string;
	readonly cause: unknown;
}> {}

function workflowFailure(message: string, cause: unknown): ToolchainValidationWorkflowFailed {
	return new ToolchainValidationWorkflowFailed({ message, cause });
}

function workflowPromise<A>(
	evaluate: () => PromiseLike<A>,
	message: string,
): AppEffect<A, ToolchainValidationWorkflowFailed> {
	return Effect.tryPromise({
		try: evaluate,
		catch: (cause) => workflowFailure(message, cause),
	});
}

function hydrateAvailability(
	mode: ToolchainHydrationMode,
): AppEffect<void, never, ToolchainValidationWorkflowServicesId> {
	return Effect.gen(function* () {
		const services = yield* ToolchainValidationWorkflowServicesTag;
		const settings = services.readToolchainSettingsFromState();
		const availability = yield* workflowPromise(
			() =>
				mode === 'refresh'
					? services.refreshExternalToolchain(settings)
					: services.listAvailableEncoders(settings),
			'Failed to load encoder availability.',
		).pipe(
			Effect.catchAll((error) =>
				Effect.sync(() => {
					services.console.warn('Failed to load encoder availability', error.cause);
					return null;
				}),
			),
		);

		services.console.log('Encoder availability:', availability);
		services.setEncoderAvailability(availability);
		services.syncAfterAvailabilityChange();
		services.console.log('Encoder panel ready');
	});
}

function browseToolchain(): AppEffect<void, never, ToolchainValidationWorkflowServicesId> {
	return Effect.gen(function* () {
		const services = yield* ToolchainValidationWorkflowServicesTag;
		const selected = yield* workflowPromise(
			() => services.openFile({ title: 'Select ffmpeg executable' }),
			'Failed to choose ffmpeg executable.',
		).pipe(
			Effect.catchAll((error) =>
				Effect.sync(() => {
					services.console.warn('Failed to choose ffmpeg executable', error.cause);
					return null;
				}),
			),
		);
		if (!selected) {
			return;
		}

		services.setExternalToolchainOverridePath(selected);
		services.syncAfterToolchainPathChange();
		yield* hydrateAvailability('refresh');
	});
}

function toolchainValidationWorkflowBody(
	action: ToolchainValidationWorkflowAction,
): AppEffect<void, never, ToolchainValidationWorkflowServicesId> {
	return Effect.gen(function* () {
		const services = yield* ToolchainValidationWorkflowServicesTag;
		switch (action.type) {
			case 'hydrateAvailability':
				yield* hydrateAvailability(action.mode ?? 'initial');
				return;
			case 'browseToolchain':
				yield* browseToolchain();
				return;
			case 'clearOverride':
				services.setExternalToolchainOverridePath('');
				services.syncAfterToolchainPathChange();
				yield* hydrateAvailability('refresh');
				return;
			case 'commitOverride':
			case 'refresh':
				yield* hydrateAvailability('refresh');
				return;
		}
	});
}

async function defaultToolchainValidationWorkflowLayer(): Promise<ToolchainValidationWorkflowLayer> {
	const live = await import('./toolchainValidationWorkflowLive');
	return live.ToolchainValidationWorkflowLive;
}

export function toolchainValidationWorkflowExecution(
	action: ToolchainValidationWorkflowAction,
): AppEffect<void, never, ToolchainValidationWorkflowServicesId> {
	return toolchainValidationWorkflowBody(action);
}

export async function runToolchainValidationWorkflow(
	action: ToolchainValidationWorkflowAction,
	layer?: ToolchainValidationWorkflowLayer,
): Promise<void> {
	const workflowLayer = layer ?? (await defaultToolchainValidationWorkflowLayer());
	return runAppEffect(toolchainValidationWorkflowBody(action).pipe(Effect.provide(workflowLayer)));
}
