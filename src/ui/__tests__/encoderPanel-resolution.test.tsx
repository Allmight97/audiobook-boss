import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from '@solidjs/testing-library';
import { type AppRuntime, createAppRuntime, AppRuntimeProvider } from '../../app/runtime';

import { EncoderView } from '../encoderPanel/EncoderView';
import {
	encoderAvailabilityFixture,
	runtimeSettingsCapabilitiesFixture,
} from '../../test/fixtures/runtimeSettingsCapabilities';

const context = vi.hoisted(() => ({
	getRuntimeSettingsCapabilitiesMock: vi.fn(),
}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		getRuntimeSettingsCapabilities: context.getRuntimeSettingsCapabilitiesMock,
		openFile: vi.fn(),
		updateAppSettings: vi.fn().mockResolvedValue(undefined),
	},
}));

const changeSelectValue = (select: HTMLSelectElement, value: string): void => {
	select.value = value;
	select.dispatchEvent(new Event('change', { bubbles: true }));
};

describe('encoder panel encoder resolution', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		runtime?.dispose();
		runtime = undefined;
	});

	function renderEncoder() {
		runtime?.dispose();
		runtime = createAppRuntime();
		runtime.encoding.select('intent', 'encode');
		return render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<EncoderView />
			</AppRuntimeProvider>
		));
	}

	beforeEach(() => {
		context.getRuntimeSettingsCapabilitiesMock.mockReset();
	});

	it('shows the resolved NMR encoder in Settings without an extra Auto choice', async () => {
		context.getRuntimeSettingsCapabilitiesMock.mockResolvedValue(
			runtimeSettingsCapabilitiesFixture({
				encoder: {
					availability: encoderAvailabilityFixture({
						aacAtAvailable: false,
						nativeAacAvailable: true,
					}),
				},
			}),
		);

		renderEncoder();

		await vi.waitFor(() => {
			const select = document.getElementById('adv-encoder') as HTMLSelectElement | null;
			expect(select?.value).toBe('native_aac');
			expect(select?.options.length).toBe(3);
			expect(document.getElementById('native-speed')).not.toBeNull();
		});
	});

	it('saves an explicit encoder choice from Settings', async () => {
		context.getRuntimeSettingsCapabilitiesMock.mockResolvedValue(
			runtimeSettingsCapabilitiesFixture({
				encoder: {
					availability: encoderAvailabilityFixture({
						aacAtAvailable: true,
						nativeAacAvailable: true,
					}),
				},
			}),
		);

		renderEncoder();

		await vi.waitFor(() => {
			const select = document.getElementById('adv-encoder') as HTMLSelectElement | null;
			expect(select?.value).toBe('native_aac');
		});

		const select = document.getElementById('adv-encoder') as HTMLSelectElement;
		changeSelectValue(select, 'faac');

		await vi.waitFor(() => {
			expect(select.value).toBe('faac');
			expect(runtime?.encoding.audioRequest().settings?.encoderType).toBe('faac');
		});
	});

	it('shows NMR speed when Native AAC is manually selected', async () => {
		context.getRuntimeSettingsCapabilitiesMock.mockResolvedValue(
			runtimeSettingsCapabilitiesFixture({
				encoder: {
					availability: encoderAvailabilityFixture({
						aacAtAvailable: true,
						nativeAacAvailable: true,
					}),
				},
			}),
		);

		renderEncoder();

		await vi.waitFor(() => {
			const select = document.getElementById('adv-encoder') as HTMLSelectElement | null;
			expect(select?.value).toBe('native_aac');
		});

		const select = document.getElementById('adv-encoder') as HTMLSelectElement;
		changeSelectValue(select, 'faac');
		changeSelectValue(select, 'native_aac');

		await vi.waitFor(() => {
			expect(select.value).toBe('native_aac');
			expect(document.getElementById('native-speed')).not.toBeNull();
		});
	});
});
