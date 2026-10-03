import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from '@solidjs/testing-library';
import { type AppRuntime, createAppRuntime, AppRuntimeProvider } from '../../app/runtime';
import { createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import { EncoderView } from '../encoderPanel/EncoderView';

const changeSelectValue = (select: HTMLSelectElement, value: string): void => {
	select.value = value;
	select.dispatchEvent(new Event('change', { bubbles: true }));
};

describe('encoder panel encoder resolution', () => {
	let runtime: AppRuntime | undefined;
	let engine: FakeEngine;

	afterEach(() => {
		runtime?.dispose();
		runtime = undefined;
	});

	async function renderEncoder() {
		engine = createFakeEngine();
		engine.change((state) => {
			state.audio.defaults.choice.intent = 'encode';
		});
		runtime = createAppRuntime({ engine });
		await runtime.initialize();
		return render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<EncoderView />
			</AppRuntimeProvider>
		));
	}

	it('shows the encoder the engine resolved, with its speed, and no extra Auto choice', async () => {
		await renderEncoder();

		await vi.waitFor(() => {
			const select = document.getElementById('adv-encoder') as HTMLSelectElement | null;
			expect(select?.value).toBe('native_aac');
			expect(Array.from(select?.options ?? []).map((option) => option.value)).toEqual([
				'aac_at',
				'native_aac',
				'faac',
			]);
			expect(document.getElementById('native-speed')).not.toBeNull();
		});
	});

	it('sends an explicit encoder choice and hides native speed once the engine applies it', async () => {
		await renderEncoder();
		const select = document.getElementById('adv-encoder') as HTMLSelectElement;

		changeSelectValue(select, 'faac');
		await vi.waitFor(() =>
			expect(engine.sessionIntents).toContainEqual({
				kind: 'setDefaultAudio',
				edit: { field: 'encoder', value: 'faac' },
			}),
		);
		engine.change((state) => {
			state.audio.defaults.choice.encoder = 'faac';
			state.audio.defaults.facts.effectiveEncoder = 'faac';
		});
		await vi.waitFor(() => {
			expect(select.value).toBe('faac');
			expect(document.getElementById('native-speed')).toBeNull();
		});
	});
});
