import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
	hydrateRuntimeSettingsCapabilities,
	runtimeSettingsCapabilitiesState,
} from '../runtimeSettingsCapabilities.svelte';
import { runtimeSettingsCapabilitiesFixture } from '../../test/fixtures/runtimeSettingsCapabilities';

const context = vi.hoisted(() => ({
	getRuntimeSettingsCapabilitiesMock: vi.fn(),
}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		getRuntimeSettingsCapabilities: context.getRuntimeSettingsCapabilitiesMock,
	},
}));

describe('runtime settings capability hydration', () => {
	beforeEach(() => {
		context.getRuntimeSettingsCapabilitiesMock.mockReset();
		runtimeSettingsCapabilitiesState.capabilities = null;
		runtimeSettingsCapabilitiesState.loadError = null;
		runtimeSettingsCapabilitiesState.loading = false;
	});

	it('deduplicates concurrent auto-detection loads', async () => {
		const capabilities = runtimeSettingsCapabilitiesFixture();
		context.getRuntimeSettingsCapabilitiesMock.mockResolvedValue(capabilities);

		const first = hydrateRuntimeSettingsCapabilities();
		const second = hydrateRuntimeSettingsCapabilities();

		await expect(first).resolves.toBe(capabilities);
		await expect(second).resolves.toBe(capabilities);
		expect(context.getRuntimeSettingsCapabilitiesMock).toHaveBeenCalledTimes(1);
		expect(context.getRuntimeSettingsCapabilitiesMock).toHaveBeenCalledWith();
	});

	it('uses auto-detect as the only runtime capability ingress', async () => {
		const automatic = runtimeSettingsCapabilitiesFixture();
		context.getRuntimeSettingsCapabilitiesMock.mockResolvedValueOnce(automatic);

		await hydrateRuntimeSettingsCapabilities();

		expect(context.getRuntimeSettingsCapabilitiesMock).toHaveBeenCalledTimes(1);
		expect(context.getRuntimeSettingsCapabilitiesMock).toHaveBeenCalledWith();
	});
});
