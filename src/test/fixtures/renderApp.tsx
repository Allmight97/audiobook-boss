import { cleanup, render, screen } from '@solidjs/testing-library';
import { onTestFinished } from 'vitest';
import { AppRuntimeProvider, createAppRuntime, type AppRuntime } from '../../app/runtime';
import type { InputCapability } from '../../lib/tauri/capabilities/input';
import { App } from '../../ui/App';
import type { FakeEngine } from './fakeEngine';
import { fakeInput, type FakeInput } from './fakeInput';

export async function renderApp(options: {
	engine: FakeEngine;
	input?: Partial<InputCapability>;
}): Promise<{ runtime: AppRuntime; input: FakeInput }> {
	const input = fakeInput(options.input);
	const runtime = await createAppRuntime({ input, engine: options.engine });
	onTestFinished(() => {
		cleanup();
		runtime.dispose();
	});
	render(() => (
		<AppRuntimeProvider runtime={runtime}>
			<App />
		</AppRuntimeProvider>
	));
	await screen.findByTestId('left-column');
	return { runtime, input };
}
