import { beforeEach, describe, expect, it, vi } from 'vitest';

const updateAppSettingsMock = vi.hoisted(() => vi.fn());

vi.mock('../../../lib/tauri/client', () => ({
	tauriClient: { updateAppSettings: updateAppSettingsMock },
}));

describe('app shell density', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		updateAppSettingsMock.mockResolvedValue(undefined);
		delete document.documentElement.dataset.density;
	});

	it('applies and persists a compact density selection', async () => {
		const { setDensityFromUser } = await import('../density.svelte');

		setDensityFromUser('compact');

		expect(document.documentElement.dataset.density).toBe('compact');
		await vi.waitFor(() => {
			expect(updateAppSettingsMock).toHaveBeenCalledWith({ density: 'compact' });
		});
	});

	it('omits the density attribute for a comfortable selection', async () => {
		const { applyDensityPreference } = await import('../density.svelte');
		document.documentElement.dataset.density = 'compact';

		applyDensityPreference('comfortable');

		expect(document.documentElement.hasAttribute('data-density')).toBe(false);
	});
});
