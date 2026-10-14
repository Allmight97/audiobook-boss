import { cleanup, render, screen } from '@solidjs/testing-library';
import { afterEach, expect, it, vi } from 'vitest';
import { AppRuntimeProvider, createAppRuntime, type AppRuntime } from '../../app/runtime';
import { createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import { SettingsPersistenceNotice } from '.';

let runtime: AppRuntime | undefined;
afterEach(() => {
	cleanup();
	runtime?.dispose();
	runtime = undefined;
});

async function renderNotice(recovery: FakeEngine['recovery']): Promise<FakeEngine> {
	const engine = createFakeEngine();
	engine.recovery = recovery;
	runtime = createAppRuntime({ engine });
	await runtime.initialize();
	render(() => (
		<AppRuntimeProvider runtime={runtime!}>
			<SettingsPersistenceNotice />
		</AppRuntimeProvider>
	));
	return engine;
}

it('names the settings a launch reset until the next change', async () => {
	await renderNotice({ reset: ['jobLimit', 'pinnedDefaults'] });

	const notice = screen.getByTestId('settings-recovery-notice');
	expect(notice).toHaveTextContent('number of jobs, pinned defaults');

	await runtime!.settings.setKeepAwakeWhileWorking(false);
	await vi.waitFor(() =>
		expect(screen.queryByTestId('settings-recovery-notice')).not.toBeInTheDocument(),
	);
});

it('names the file an unreadable settings file was kept as', async () => {
	await renderNotice({ reset: [], setAsideAs: 'settings.toml.unusable-17' });

	expect(screen.getByTestId('settings-recovery-notice')).toHaveTextContent(
		"The file was kept as settings.toml.unusable-17 in ABB's settings folder.",
	);
});
