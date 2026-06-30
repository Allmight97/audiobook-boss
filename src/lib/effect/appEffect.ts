import { Context, Data, Effect, Layer } from 'effect';

export { Context, Data, Effect, Layer };

export type AppEffect<A, E = never, R = never> = Effect.Effect<A, E, R>;
export type AppLayer<ROut, E = never, RIn = never> = Layer.Layer<ROut, E, RIn>;
export type AppServiceTag<Identifier, Service> = Context.Tag<Identifier, Service>;

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

export function workflowTryPromise<A, E>(
	evaluate: () => PromiseLike<A>,
	message: string,
	toFailure: (message: string, cause: unknown) => E,
): AppEffect<A, E> {
	return Effect.tryPromise({
		try: evaluate,
		catch: (cause) => toFailure(message, cause),
	});
}

export function workflowTrySync<A, E>(
	evaluate: () => A,
	message: string,
	toFailure: (message: string, cause: unknown) => E,
): AppEffect<A, E> {
	return Effect.try({
		try: evaluate,
		catch: (cause) => toFailure(message, cause),
	});
}
