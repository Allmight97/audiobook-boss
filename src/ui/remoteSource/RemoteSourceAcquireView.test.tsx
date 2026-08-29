import { cleanup, fireEvent, render } from '@solidjs/testing-library';
import { afterEach, describe, expect, it } from 'vitest';
import {
	openRemoteSourceAcquireAtom,
	remoteSourceViewAtom,
	resetRemoteSource,
} from '../../app/remoteSource';
import { AppRuntimeProvider } from '../../app/runtime/RuntimeProvider';
import { createTestAppRuntime } from '../../app/runtime/harness';
import type { AppRuntime } from '../../app/runtime';
import { RemoteSourceAcquireView } from './RemoteSourceAcquireView';

describe('RemoteSourceAcquireView close wiring', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		cleanup();
		runtime?.dispose();
		runtime = undefined;
		resetRemoteSource();
		document.body.innerHTML = '';
	});

	it('routes Escape through the same close callback the Close button uses without cancelling acquisition', async () => {
		runtime = createTestAppRuntime();
		render(() => (
			<AppRuntimeProvider registry={runtime!.registry}>
				<button type="button" id="acquire-invoker">
					Open
				</button>
				<RemoteSourceAcquireView />
			</AppRuntimeProvider>
		));
		runtime.registry.set(openRemoteSourceAcquireAtom, undefined);

		await fireEvent.keyDown(document.getElementById('remote-source-close') as Element, {
			key: 'Escape',
			bubbles: true,
		});

		expect(runtime.registry.get(remoteSourceViewAtom).isOpen).toBe(false);
	});
});
