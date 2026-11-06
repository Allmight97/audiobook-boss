import { cleanup, fireEvent, render, screen } from '@solidjs/testing-library';
import { afterEach, expect, it, vi } from 'vitest';
import { AppRuntimeProvider, createAppRuntime, type AppRuntime } from '../../app/runtime';
import { fakeEngine } from '../../test/fixtures/fakeEngine';
import { ConcurrencyControl } from '.';
import { SettingsPersistenceNotice } from '../appSettings';

let runtime: AppRuntime | undefined;
afterEach(() => {
	cleanup();
	runtime?.dispose();
});

it('restores the accepted select value and shows the engine rejection', async () => {
	const engine = fakeEngine();
	runtime = await createAppRuntime();
	render(() => (
		<AppRuntimeProvider runtime={runtime!}>
			<ConcurrencyControl />
			<SettingsPersistenceNotice />
		</AppRuntimeProvider>
	));
	const select = screen.getByRole('combobox') as HTMLSelectElement;
	expect(select).toHaveValue('auto');
	expect(select.options[0]?.textContent).toBe('Auto · 4 jobs');

	engine.respondSettings = () => ({
		kind: 'rejected',
		error: { code: 'io_error', category: 'io', message: 'Jobs active', detail: null },
	});
	fireEvent.change(select, { target: { value: '3' } });
	await vi.waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Jobs active'));
	expect(select).toHaveValue('auto');

	engine.respondSettings = () => {
		const { concurrency } = engine.settings();
		engine.seedSettings({
			concurrency: { ...concurrency, preference: { mode: 'fixed', value: 3 }, effective: 3 },
		});
		return undefined;
	};
	fireEvent.change(select, { target: { value: '3' } });
	await vi.waitFor(() => expect(runtime!.settings.concurrency().effective).toBe(3));
	expect(select).toHaveValue('3');
	expect(select.options[0]?.textContent).toBe('Auto · 4 jobs');
	expect(engine.settingsIntents).toContainEqual({
		kind: 'setConcurrency',
		preference: { mode: 'fixed', value: 3 },
	});
});
