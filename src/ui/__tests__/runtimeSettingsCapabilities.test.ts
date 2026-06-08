import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
	hydrateRuntimeSettingsCapabilities,
	runtimeSettingsCapabilitiesState,
	setRuntimeSettingsCapabilities,
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
		setRuntimeSettingsCapabilities(null);
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

	it('stores capabilities in shared state after successful hydration', async () => {
		const capabilities = runtimeSettingsCapabilitiesFixture();
		context.getRuntimeSettingsCapabilitiesMock.mockResolvedValue(capabilities);

		await hydrateRuntimeSettingsCapabilities();

		expect(runtimeSettingsCapabilitiesState.capabilities).toStrictEqual(capabilities);
		expect(runtimeSettingsCapabilitiesState.loadError).toBeNull();
		expect(runtimeSettingsCapabilitiesState.loading).toBe(false);
	});

	it('reuses a successful hydration result for later consumers', async () => {
		const capabilities = runtimeSettingsCapabilitiesFixture();
		context.getRuntimeSettingsCapabilitiesMock.mockResolvedValue(capabilities);

		await hydrateRuntimeSettingsCapabilities();
		await hydrateRuntimeSettingsCapabilities();

		expect(context.getRuntimeSettingsCapabilitiesMock).toHaveBeenCalledTimes(1);
		expect(runtimeSettingsCapabilitiesState.capabilities).toStrictEqual(capabilities);
	});

	it('sets error state on failed load and clears capabilities', async () => {
		const failure = new Error('backend unavailable');
		context.getRuntimeSettingsCapabilitiesMock.mockRejectedValue(failure);

		await hydrateRuntimeSettingsCapabilities();

		expect(runtimeSettingsCapabilitiesState.capabilities).toBeNull();
		expect(runtimeSettingsCapabilitiesState.loadError).toBe(failure);
		expect(runtimeSettingsCapabilitiesState.loading).toBe(false);
	});

	it('allows consumers to reuse shared state without additional fetches', () => {
		const capabilities = runtimeSettingsCapabilitiesFixture();
		setRuntimeSettingsCapabilities(capabilities);

		expect(runtimeSettingsCapabilitiesState.capabilities).toStrictEqual(capabilities);
		expect(context.getRuntimeSettingsCapabilitiesMock).not.toHaveBeenCalled();
	});

	it('clears error state when explicit capability set is called', () => {
		runtimeSettingsCapabilitiesState.loadError = new Error('stale');
		const capabilities = runtimeSettingsCapabilitiesFixture();

		setRuntimeSettingsCapabilities(capabilities);

		expect(runtimeSettingsCapabilitiesState.loadError).toBeNull();
		expect(runtimeSettingsCapabilitiesState.capabilities).toStrictEqual(capabilities);
	});
});
