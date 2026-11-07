import { createSignal, onCleanup, Show } from 'solid-js';
import type { JSX } from '@solidjs/web';

import { toUserMessage } from '../../lib/tauri/appError';
import { AppRuntimeProvider } from './context';
import { createAppRuntime } from './index';
import type { AppRuntime, RuntimeCapabilities } from './types';

export function AppRoot(props: {
	readonly capabilities?: RuntimeCapabilities;
	readonly children: JSX.Element;
}): JSX.Element {
	const [runtime, setRuntime] = createSignal<AppRuntime>();
	const [failure, setFailure] = createSignal('');
	let mounted = true;
	let started: AppRuntime | undefined;
	async function start(capabilities?: RuntimeCapabilities): Promise<void> {
		try {
			started = await createAppRuntime(capabilities);
		} catch (error) {
			setFailure(toUserMessage(error, { fallback: 'ABB could not reach its engine.' }));
			return;
		}
		if (mounted) setRuntime(started);
		else started.dispose();
	}
	void start(props.capabilities);
	onCleanup(() => {
		mounted = false;
		started?.dispose();
	});
	return (
		<Show
			when={runtime()}
			fallback={
				<Show when={failure()}>
					<p class="engine-attach-failed" role="alert">
						{failure()}
					</p>
				</Show>
			}
		>
			{(attached) => <AppRuntimeProvider runtime={attached()}>{props.children}</AppRuntimeProvider>}
		</Show>
	);
}
