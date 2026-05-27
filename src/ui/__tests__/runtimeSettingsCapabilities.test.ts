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

	it('deduplicates concurrent loads for the same explicit toolchain key', async () => {
		const capabilities = runtimeSettingsCapabilitiesFixture();
		context.getRuntimeSettingsCapabilitiesMock.mockResolvedValue(capabilities);

		const first = hydrateRuntimeSettingsCapabilities(null);
		const second = hydrateRuntimeSettingsCapabilities(null);

		await expect(first).resolves.toBe(capabilities);
		await expect(second).resolves.toBe(capabilities);
		expect(context.getRuntimeSettingsCapabilitiesMock).toHaveBeenCalledTimes(1);
		expect(context.getRuntimeSettingsCapabilitiesMock).toHaveBeenCalledWith(null);
	});

	it('keeps separate pending loads for distinct explicit toolchain keys', async () => {
		const automatic = runtimeSettingsCapabilitiesFixture();
		const override = runtimeSettingsCapabilitiesFixture({
			encoder: {
				availability: {
					...runtimeSettingsCapabilitiesFixture().encoder.availability,
					overrideToolchainPath: '/custom/ffmpeg',
					activeToolchainPath: '/custom/ffmpeg',
				},
			},
		});
		context.getRuntimeSettingsCapabilitiesMock
			.mockResolvedValueOnce(automatic)
			.mockResolvedValueOnce(override);

		await hydrateRuntimeSettingsCapabilities(null);
		await hydrateRuntimeSettingsCapabilities({ overridePath: '/custom/ffmpeg' });

		expect(context.getRuntimeSettingsCapabilitiesMock).toHaveBeenCalledTimes(2);
		expect(context.getRuntimeSettingsCapabilitiesMock).toHaveBeenNthCalledWith(1, null);
		expect(context.getRuntimeSettingsCapabilitiesMock).toHaveBeenNthCalledWith(2, {
			overridePath: '/custom/ffmpeg',
		});
	});
});
