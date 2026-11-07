import { Show } from 'solid-js';
import type { JSX } from '@solidjs/web';
import { useAppRuntime } from '../app/runtime';
import './quitReminder.css';

export function QuitReminder(): JSX.Element {
	const engine = useAppRuntime().engine;
	return (
		<Show when={engine.quitReminder()}>
			<div class="quit-reminder" role="status" aria-live="polite">
				Press ⌘Q again to quit
			</div>
		</Show>
	);
}
