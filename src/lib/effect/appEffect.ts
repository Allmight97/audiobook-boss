import { Context, Data, Effect, Layer } from 'effect';

export { Context, Data, Effect, Layer };

export type AppEffect<A, E = never, R = never> = Effect.Effect<A, E, R>;
export type AppLayer<ROut, E = never, RIn = never> = Layer.Layer<ROut, E, RIn>;
export type AppServiceTag<Identifier, Service> = Context.Tag<Identifier, Service>;

export interface WorkflowErrorDetails {
	readonly message: string;
	readonly cause?: unknown;
}

export class UnexpectedWorkflowError extends Data.TaggedError(
	'UnexpectedWorkflowError',
)<WorkflowErrorDetails> {}

export function makeWorkflowServiceTag<Identifier extends string, Service>(
	identifier: Identifier,
): AppServiceTag<Identifier, Service> {
	return Context.GenericTag<Identifier, Service>(`abb/${identifier}`);
}

export function makeWorkflowLayer<Identifier, Service>(
	tag: AppServiceTag<Identifier, Service>,
	service: Service,
): AppLayer<Identifier> {
	return Layer.succeed(tag, service);
}

export function runAppEffect<A, E>(program: AppEffect<A, E, never>): Promise<A> {
	return Effect.runPromise(program);
}

export function unknownErrorMessage(error: unknown): string {
	if (error instanceof Error && error.message.length > 0) {
		return error.message;
	}
	if (typeof error === 'string' && error.length > 0) {
		return error;
	}
	return 'Unexpected workflow error.';
}

export function unexpectedWorkflowError(message: string, cause?: unknown): UnexpectedWorkflowError {
	return new UnexpectedWorkflowError({ message, cause });
}

export function tryAppSync<A>(
	evaluate: () => A,
	message = 'Unexpected workflow error.',
): AppEffect<A, UnexpectedWorkflowError> {
	return Effect.try({
		try: evaluate,
		catch: (cause) => unexpectedWorkflowError(message, cause),
	});
}

export function tryAppPromise<A>(
	evaluate: () => PromiseLike<A>,
	message = 'Unexpected workflow error.',
): AppEffect<A, UnexpectedWorkflowError> {
	return Effect.tryPromise({
		try: evaluate,
		catch: (cause) => unexpectedWorkflowError(message, cause),
	});
}
