import { cleanup, fireEvent, render, screen } from '@solidjs/testing-library';
import { afterEach, expect, it, vi } from 'vitest';
import { AppRuntimeProvider, createAppRuntime, type AppRuntime } from '../../app/runtime';
import { tauriClient } from '../../lib/tauri/client';
import { ConcurrencyControl } from '.';
import { SettingsPersistenceNotice } from '../appSettings';

let runtime: AppRuntime | undefined;
afterEach(() => {
	cleanup();
	runtime?.dispose();
	vi.restoreAllMocks();
});

it('restores the accepted select value and shows the runtime rejection', async () => {
	runtime = createAppRuntime();
	const setMaxJobs = vi.spyOn(tauriClient, 'setMaxConcurrentJobs').mockResolvedValue(4);
	await runtime.settings.hydrateConcurrency();
	setMaxJobs.mockRejectedValueOnce(new Error('Jobs active'));
	render(() => (
		<AppRuntimeProvider runtime={runtime!}>
			<ConcurrencyControl />
			<SettingsPersistenceNotice />
		</AppRuntimeProvider>
	));
	const select = screen.getByRole('combobox') as HTMLSelectElement;
	expect(select).toHaveValue('auto');
	expect(select.options[0]?.textContent).toBe('Auto · 4 jobs');
	await fireEvent.change(select, { target: { value: '3' } });
	await vi.waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Jobs active'));
	expect(select).toHaveValue('auto');
	setMaxJobs.mockResolvedValueOnce(3);
	await fireEvent.change(select, { target: { value: '3' } });
	await vi.waitFor(() => expect(runtime!.settings.concurrency().effective).toBe(3));
	expect(select).toHaveValue('3');
	expect(select.options[0]?.textContent).toBe('Auto · 4 jobs');
});
