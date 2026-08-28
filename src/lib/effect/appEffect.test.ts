import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { Data } from './appEffect';
import {
	type AppEffect,
	Effect,
	makeWorkflowKit,
	makeWorkflowLayer,
	makeWorkflowServiceTag,
	runAppEffect,
	workflowTryPromise,
	workflowTrySync,
} from './appEffect';

class HarnessWorkflowFailed extends Data.TaggedError('HarnessWorkflowFailed')<{
	readonly message: string;
	readonly cause: unknown;
}> {}

function harnessFailure(message: string, cause: unknown): HarnessWorkflowFailed {
	return new HarnessWorkflowFailed({ message, cause });
}

describe('AppEffect kernel', () => {
	it('runs Svelte-callable promise bridges with provided workflow services', async () => {
		const NumberService = makeWorkflowServiceTag<'test/NumberService', { value: number }>(
			'test/NumberService',
		);
		const program = Effect.gen(function* () {
			const service = yield* NumberService;
			return service.value * 2;
		});

		const result = await runAppEffect(
			program.pipe(Effect.provide(makeWorkflowLayer(NumberService, { value: 21 }))),
		);

		expect(result).toBe(42);
	});

	it('maps rejected promises into owner-specific workflow errors via workflowTryPromise', async () => {
		const error = await runAppEffect(
			workflowTryPromise(
				() => Promise.reject(new Error('owner dependency failed')),
				'Owner workflow dependency failed.',
				harnessFailure,
			).pipe(Effect.flip),
		);

		expect(error).toMatchObject({
			_tag: 'HarnessWorkflowFailed',
			message: 'Owner workflow dependency failed.',
			cause: expect.any(Error),
		} satisfies Partial<HarnessWorkflowFailed>);
	});

	it('maps synchronous throws into owner-specific workflow errors via workflowTrySync', async () => {
		const error = await runAppEffect(
			workflowTrySync(
				() => {
					throw new Error('sync dependency failed');
				},
				'Owner sync workflow failed.',
				harnessFailure,
			).pipe(Effect.flip),
		);

		expect(error).toMatchObject({
			_tag: 'HarnessWorkflowFailed',
			message: 'Owner sync workflow failed.',
			cause: expect.any(Error),
		} satisfies Partial<HarnessWorkflowFailed>);
	});
});

describe('makeWorkflowKit (#389 spike acceptance)', () => {
	type AlphaServices = { readonly label: string };
	type BetaServices = { readonly label: string };

	const alphaKit = makeWorkflowKit(
		'Test/AlphaWorkflowServices',
		'AlphaWorkflowFailed',
	)<AlphaServices>();
	const betaKit = makeWorkflowKit(
		'Test/BetaWorkflowServices',
		'BetaWorkflowFailed',
	)<BetaServices>();

	it('catchTag discriminates one kit failure from another', async () => {
		type AlphaFailed = InstanceType<typeof alphaKit.Failed>;
		type BetaFailed = InstanceType<typeof betaKit.Failed>;
		// A program whose failure union spans two owners — the catchTag calls
		// below typecheck against the union and must fire only for the owner
		// whose tag matches, exactly like hand-written failure classes.
		const failing: AppEffect<string, AlphaFailed | BetaFailed> = alphaKit.trySync(() => {
			throw new Error('alpha dependency failed');
		}, 'Alpha workflow failed.');

		const caught = await runAppEffect(
			failing.pipe(
				// The other owner's tag must NOT catch this failure...
				Effect.catchTag('BetaWorkflowFailed', () => Effect.succeed('wrong-owner-caught')),
				// ...while the owning tag must.
				Effect.catchTag('AlphaWorkflowFailed', (error) =>
					Effect.succeed(`alpha-caught:${error.message}`),
				),
			),
		);

		expect(caught).toBe('alpha-caught:Alpha workflow failed.');
	});

	it('kit failures keep distinct identities and instanceof behavior', async () => {
		const alphaError = await runAppEffect(
			alphaKit
				.tryPromise(() => Promise.reject(new Error('boom')), 'Alpha async failed.')
				.pipe(Effect.flip),
		);

		expect(alphaError).toBeInstanceOf(alphaKit.Failed);
		expect(alphaError).not.toBeInstanceOf(betaKit.Failed);
		expect(alphaError._tag).toBe('AlphaWorkflowFailed');
	});

	it('kit layer provides the service through the kit tag', async () => {
		const program = Effect.gen(function* () {
			const services = yield* alphaKit.Tag;
			return services.label;
		});

		const result = await runAppEffect(
			program.pipe(Effect.provide(alphaKit.makeLive({ label: 'alpha-live' }))),
		);

		expect(result).toBe('alpha-live');
	});
});

function importsEffectPackage(source: string): boolean {
	return /\b(?:from\s+|import\s*\(\s*|require\s*\(\s*|import\s+)['"]effect(?:\/[^'"]*)?['"]/.test(
		source,
	);
}

describe('Effect package import boundary', () => {
	const srcRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
	const allowed = path.normalize(path.join(srcRoot, 'lib/effect/appEffect.ts'));

	it('treats Effect package root and subpath specifiers as the same boundary', () => {
		expect(importsEffectPackage("import { Effect } from 'effect'")).toBe(true);
		expect(importsEffectPackage("import { Effect } from 'effect/Effect'")).toBe(true);
		expect(importsEffectPackage("import { something } from './effect'")).toBe(false);
	});

	it('records effect specifiers from multiline named imports', () => {
		expect(importsEffectPackage("import {\n\tEffect,\n} from 'effect';\n")).toBe(true);
		expect(importsEffectPackage("import { Effect } from './effect';\n")).toBe(false);
	});

	it('lets only appEffect.ts import the effect package', () => {
		const hits: string[] = [];
		const self = path.normalize(fileURLToPath(import.meta.url));
		const walk = (dir: string): void => {
			for (const entry of readdirSync(dir, { withFileTypes: true })) {
				const fullPath = path.join(dir, entry.name);
				if (entry.isDirectory()) {
					if (entry.name === 'node_modules' || entry.name === 'dist') {
						continue;
					}
					walk(fullPath);
					continue;
				}
				if (!/\.(?:[cm]?tsx?|mjs|svelte)$/.test(entry.name)) {
					continue;
				}
				if (path.normalize(fullPath) === allowed || path.normalize(fullPath) === self) {
					continue;
				}
				const source = readFileSync(fullPath, 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
				if (importsEffectPackage(source)) {
					hits.push(path.relative(srcRoot, fullPath));
				}
			}
		};
		walk(srcRoot);
		expect(hits).toEqual([]);
	});
});
