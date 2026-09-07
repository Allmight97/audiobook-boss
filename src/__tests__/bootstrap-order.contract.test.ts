import { afterEach, expect, it, vi } from 'vitest';

const evaluations = vi.hoisted(() => [] as string[]);
vi.mock('../lib/frontendLogBridge.install', () => {
	evaluations.push('log bridge');
	return {};
});
vi.mock('../app/runtime/ProductionRoot', () => {
	evaluations.push('app root');
	return { ProductionRoot: () => null };
});
vi.mock('@solidjs/web', () => ({ render: () => () => {} }));

afterEach(() => vi.unstubAllGlobals());

it('evaluates the frontend log bridge before importing the app root', async () => {
	vi.stubGlobal('document', { getElementById: () => ({}) });
	await import('../main');
	expect(evaluations).toEqual(['log bridge', 'app root']);
});
