import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
	invalidateRuntimeSettingsCapabilities,
	loadRuntimeSettingsCapabilities,
	refreshRuntimeSettingsCapabilities,
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

describe('runtime settings capability loading', () => {
	beforeEach(() => {
		context.getRuntimeSettingsCapabilitiesMock.mockReset();
		invalidateRuntimeSettingsCapabilities();
	});

	it('deduplicates concurrent auto-detection loads', async () => {
		const capabilities = runtimeSettingsCapabilitiesFixture();
		context.getRuntimeSettingsCapabilitiesMock.mockResolvedValue(capabilities);

		const first = loadRuntimeSettingsCapabilities();
		const second = loadRuntimeSettingsCapabilities();

		await expect(first).resolves.toBe(capabilities);
		await expect(second).resolves.toBe(capabilities);
		expect(context.getRuntimeSettingsCapabilitiesMock).toHaveBeenCalledTimes(1);
		expect(context.getRuntimeSettingsCapabilitiesMock).toHaveBeenCalledWith();
	});

	it('uses auto-detect as the only runtime capability ingress', async () => {
		const automatic = runtimeSettingsCapabilitiesFixture();
		context.getRuntimeSettingsCapabilitiesMock.mockResolvedValueOnce(automatic);

		await loadRuntimeSettingsCapabilities();

		expect(context.getRuntimeSettingsCapabilitiesMock).toHaveBeenCalledTimes(1);
		expect(context.getRuntimeSettingsCapabilitiesMock).toHaveBeenCalledWith();
	});

	it('stores capabilities in shared state after successful load', async () => {
		const capabilities = runtimeSettingsCapabilitiesFixture();
		context.getRuntimeSettingsCapabilitiesMock.mockResolvedValue(capabilities);

		await loadRuntimeSettingsCapabilities();

		expect(runtimeSettingsCapabilitiesState.capabilities).toStrictEqual(capabilities);
		expect(runtimeSettingsCapabilitiesState.loadError).toBeNull();
		expect(runtimeSettingsCapabilitiesState.loading).toBe(false);
	});

	it('reuses a successful load result for later consumers', async () => {
		const capabilities = runtimeSettingsCapabilitiesFixture();
		context.getRuntimeSettingsCapabilitiesMock.mockResolvedValue(capabilities);

		await loadRuntimeSettingsCapabilities();
		await loadRuntimeSettingsCapabilities();

		expect(context.getRuntimeSettingsCapabilitiesMock).toHaveBeenCalledTimes(1);
		expect(runtimeSettingsCapabilitiesState.capabilities).toStrictEqual(capabilities);
	});

	it('refreshes by bypassing a cached successful load', async () => {
		const cached = runtimeSettingsCapabilitiesFixture({
			maxConcurrentJobs: { fixedOptions: [1, 2] },
		});
		const refreshed = runtimeSettingsCapabilitiesFixture({
			maxConcurrentJobs: { fixedOptions: [1, 2, 3, 4] },
		});
		context.getRuntimeSettingsCapabilitiesMock
			.mockResolvedValueOnce(cached)
			.mockResolvedValueOnce(refreshed);

		await loadRuntimeSettingsCapabilities();
		await refreshRuntimeSettingsCapabilities();

		expect(context.getRuntimeSettingsCapabilitiesMock).toHaveBeenCalledTimes(2);
		expect(runtimeSettingsCapabilitiesState.capabilities).toStrictEqual(refreshed);
	});

	it('deduplicates concurrent refresh loads', async () => {
		const refreshed = runtimeSettingsCapabilitiesFixture();
		context.getRuntimeSettingsCapabilitiesMock.mockResolvedValue(refreshed);

		const first = refreshRuntimeSettingsCapabilities();
		const second = refreshRuntimeSettingsCapabilities();

		await expect(first).resolves.toBe(refreshed);
		await expect(second).resolves.toBe(refreshed);
		expect(context.getRuntimeSettingsCapabilitiesMock).toHaveBeenCalledTimes(1);
	});

	it('joins an in-flight refresh instead of returning stale cached capabilities', async () => {
		const cached = runtimeSettingsCapabilitiesFixture({
			maxConcurrentJobs: { fixedOptions: [1, 2] },
		});
		const refreshed = runtimeSettingsCapabilitiesFixture({
			maxConcurrentJobs: { fixedOptions: [1, 2, 3, 4] },
		});
		context.getRuntimeSettingsCapabilitiesMock
			.mockResolvedValueOnce(cached)
			.mockResolvedValueOnce(refreshed);

		await loadRuntimeSettingsCapabilities();
		const refresh = refreshRuntimeSettingsCapabilities();
		const joined = loadRuntimeSettingsCapabilities();

		await expect(refresh).resolves.toBe(refreshed);
		await expect(joined).resolves.toBe(refreshed);
		expect(context.getRuntimeSettingsCapabilitiesMock).toHaveBeenCalledTimes(2);
	});

	it('invalidates cached capabilities and load state', async () => {
		const capabilities = runtimeSettingsCapabilitiesFixture();
		context.getRuntimeSettingsCapabilitiesMock.mockResolvedValue(capabilities);

		await loadRuntimeSettingsCapabilities();
		runtimeSettingsCapabilitiesState.loadError = new Error('stale');
		runtimeSettingsCapabilitiesState.loading = true;

		invalidateRuntimeSettingsCapabilities();

		expect(runtimeSettingsCapabilitiesState.capabilities).toBeNull();
		expect(runtimeSettingsCapabilitiesState.loadError).toBeNull();
		expect(runtimeSettingsCapabilitiesState.loading).toBe(false);
	});

	it('sets error state on failed load and clears capabilities', async () => {
		const failure = new Error('backend unavailable');
		context.getRuntimeSettingsCapabilitiesMock.mockRejectedValue(failure);

		await loadRuntimeSettingsCapabilities();

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
