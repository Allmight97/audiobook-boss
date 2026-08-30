import { cleanup, fireEvent, render } from '@solidjs/testing-library';
import { afterEach, describe, expect, it } from 'vitest';
import { lookupViewAtom } from '../../../app/metadataLookup';
import {
	metadataLookupState,
	snapshotMetadataLookupState,
} from '../../../app/metadataLookup/state';
import { AppRuntimeProvider } from '../../../app/runtime/RuntimeProvider';
import { createTestAppRuntime } from '../../../app/runtime/harness';
import type { AppRuntime } from '../../../app/runtime';
import { MetadataLookupView } from '../MetadataLookupView';

describe('MetadataLookupView modal wiring', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		cleanup();
		runtime?.dispose();
		runtime = undefined;
		document.body.innerHTML = '';
	});

	it('routes Escape through the same close callback the Close button uses', async () => {
		runtime = createTestAppRuntime();
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<button type="button" id="lookup-invoker">
					Open
				</button>
				<MetadataLookupView />
			</AppRuntimeProvider>
		));
		metadataLookupState.isOpen = true;
		runtime.registry.set(lookupViewAtom, snapshotMetadataLookupState());

		await fireEvent.keyDown(document.getElementById('metadata-lookup-close') as Element, {
			key: 'Escape',
			bubbles: true,
		});

		expect(metadataLookupState.isOpen).toBe(false);
	});
});
