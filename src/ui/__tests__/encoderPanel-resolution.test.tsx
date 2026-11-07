import { afterEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@solidjs/testing-library';
import { type AppRuntime, createAppRuntime, AppRuntimeProvider } from '../../app/runtime';
import { recordedFresh } from '../../lib/tauri/recordedFresh';
import { createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import { EncoderView } from '../encoderPanel';

const changeSelectValue = (select: HTMLSelectElement, value: string): void => {
	select.value = value;
	select.dispatchEvent(new Event('change', { bubbles: true }));
};

const recordedAudio = recordedFresh.session.audio;
if (!recordedAudio) throw new Error('the recorded fresh session has no audio');
const { facts } = recordedAudio.defaults;

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
		runtime = await createAppRuntime({ engine });
		return render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<EncoderView scope={{ kind: 'defaults' }} />
			</AppRuntimeProvider>
		));
	}

	it('shows the encoder the engine resolved, with its speed, and no extra Auto choice', async () => {
		await renderEncoder();

		await vi.waitFor(() => {
			const select = screen.getByTestId<HTMLSelectElement>('encoder-select');
			expect(select.value).toBe(facts.effectiveEncoder);
			expect(Array.from(select.options).map((option) => option.value)).toEqual(
				facts.encoderOptions.map((option) => option.encoder),
			);
			expect(screen.queryByTestId('native-speed-select')).not.toBeNull();
		});
	});

	it('sends an explicit encoder choice and hides native speed once the engine applies it', async () => {
		await renderEncoder();
		const select = screen.getByTestId<HTMLSelectElement>('encoder-select');

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
			expect(screen.queryByTestId('native-speed-select')).toBeNull();
		});
	});
});
