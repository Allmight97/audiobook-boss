import { afterEach, describe, expect, it } from 'vitest';
import { createAppRuntime } from './index';
import { inputSessionAtom } from '../inputSession/atoms';
import { emptyInputSession } from '../inputSession/types';
import { patchRemoteSourceState, snapshotRemoteSourceView } from '../remoteSource/state';

describe('app runtime', () => {
	let dispose: (() => void) | undefined;

	afterEach(() => {
		dispose?.();
		dispose = undefined;
	});

	it('disposes the registry so later reads do not share session state', () => {
		const first = createAppRuntime();
		first.registry.set(inputSessionAtom, {
			...emptyInputSession(),
			errorMessage: 'stale',
		});
		expect(first.registry.get(inputSessionAtom).errorMessage).toBe('stale');
		first.dispose();

		const second = createAppRuntime();
		dispose = () => second.dispose();
		expect(second.registry.get(inputSessionAtom).errorMessage).toBe('');
	});

	it('resets Remote Source module state on dispose so a remount does not keep the dialog open', () => {
		const runtime = createAppRuntime();
		patchRemoteSourceState({ isOpen: true, statusMessage: 'stale remote' });
		expect(snapshotRemoteSourceView().isOpen).toBe(true);
		runtime.dispose();
		expect(snapshotRemoteSourceView().isOpen).toBe(false);
		expect(snapshotRemoteSourceView().statusMessage).toBe('');
	});
});
