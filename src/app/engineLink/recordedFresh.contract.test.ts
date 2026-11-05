import { afterEach, describe, expect, it } from 'vitest';
import type { EngineCapability } from '../../lib/tauri/capabilities/engine';
import { recordedFresh } from '../../lib/tauri/recordedFresh';
import { createEngineLink, type EngineLink } from './link';

const neverAttaching: EngineCapability = {
	attach: () => new Promise(() => undefined),
	sessionDispatch: () => Promise.reject(new Error('not attached')),
	openPreviewOutput: () => Promise.reject(new Error('not attached')),
	settingsDispatch: () => Promise.reject(new Error('not attached')),
	listenSessionUpdates: () => Promise.resolve(() => undefined),
	listenSettingsUpdates: () => Promise.resolve(() => undefined),
};

describe('the first frame before the engine attaches', () => {
	let link: EngineLink | undefined;
	afterEach(() => link?.dispose());

	it('shows the audio defaults, output naming, and concurrency a fresh engine reports', () => {
		link = createEngineLink(neverAttaching);
		const { audio, output } = recordedFresh.session;

		expect(link.audio().defaults.choice).toEqual(audio?.defaults.choice);
		expect(link.audio().defaults.request).toEqual(audio?.defaults.request);
		expect({ ...link.output(), revision: output?.revision }).toEqual(output);
		expect(link.settings().concurrency.preference).toEqual(
			recordedFresh.settings.concurrency.preference,
		);
	});
});
