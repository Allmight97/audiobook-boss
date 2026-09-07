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
	await runtime.settings.hydrateConcurrency();
	vi.spyOn(tauriClient, 'setMaxConcurrentJobs').mockRejectedValueOnce(new Error('Jobs active'));
	render(() => (
		<AppRuntimeProvider runtime={runtime!}>
			<ConcurrencyControl />
			<SettingsPersistenceNotice />
		</AppRuntimeProvider>
	));
	const select = screen.getByRole('combobox');
	expect(select).toHaveValue('auto');
	await fireEvent.change(select, { target: { value: '3' } });
	await vi.waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Jobs active'));
	expect(select).toHaveValue('auto');
});
