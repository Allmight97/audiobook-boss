import { Context, Data, Effect, Layer } from 'effect';

export { Context, Data, Effect, Layer };

export type AppEffect<A, E = never, R = never> = Effect.Effect<A, E, R>;
export type AppLayer<ROut, E = never, RIn = never> = Layer.Layer<ROut, E, RIn>;
export type AppServiceTag<Identifier, Service> = Context.Service<Identifier, Service>;

export function makeWorkflowServiceTag<Identifier extends string, Service>(
	identifier: Identifier,
): AppServiceTag<Identifier, Service> {
	return Context.Service<Identifier, Service>(`abb/${identifier}`);
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

/**
 * One kit per workflow owner: the service tag, live-layer factory, tagged
 * failure class, failure factory, and try-wrappers that every owner used to
 * hand-copy (#389). The failure tag stays a per-owner string literal so
 * `Effect.catchTag` discriminates one owner's failure from another's exactly
 * like the hand-written classes did.
 *
 * Curried so the service type can be supplied without erasing the literal
 * types of `serviceId` / `failureTag` (TS has no partial inference):
 *
 *   const kit = makeWorkflowKit(
 *     'Core/MetadataSaveWorkflowServices',
 *     'MetadataSaveWorkflowFailed',
 *   )<MetadataSaveWorkflowServices>();
 */
export function makeWorkflowKit<const ServiceId extends string, const FailureTag extends string>(
	serviceId: ServiceId,
	failureTag: FailureTag,
) {
	return <Service>() => {
		class Failed extends Data.TaggedError(failureTag)<{
			readonly message: string;
			readonly cause: unknown;
		}> {}
		const Tag = makeWorkflowServiceTag<ServiceId, Service>(serviceId);
		const failure = (message: string, cause: unknown) => new Failed({ message, cause });
		return {
			Tag,
			Failed,
			failure,
			makeLive: (service: Service): AppLayer<ServiceId> => makeWorkflowLayer(Tag, service),
			tryPromise: <A>(evaluate: () => PromiseLike<A>, message: string) =>
				workflowTryPromise(evaluate, message, failure),
			trySync: <A>(evaluate: () => A, message: string) =>
				workflowTrySync(evaluate, message, failure),
		};
	};
}
