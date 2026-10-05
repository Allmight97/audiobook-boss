import { Show } from 'solid-js';
import type { JSX } from '@solidjs/web';
import { useAppRuntime } from '../app/runtime';
import { Button } from './foundation';
import './refusedChangeNotice.css';

/** Shows why a change was refused when nothing else waited for its answer. */
export function RefusedChangeNotice(): JSX.Element {
	const engine = useAppRuntime().engine;
	return (
		<Show when={engine.refusal()}>
			{(message) => (
				<div class="refused-change-notice" role="status" aria-live="polite">
					<p>{message()}</p>
					<Button onClick={() => engine.dismissRefusal()}>Dismiss</Button>
				</div>
			)}
		</Show>
	);
}
