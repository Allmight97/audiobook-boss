import { For } from 'solid-js';
import type { JSX } from '@solidjs/web';

import { useAppRuntime } from '../../app/runtime';
import './jobControls.css';

export function ConcurrencyControl(): JSX.Element {
	const settings = useAppRuntime().settings;
	const view = settings.concurrency;
	const setSelection = settings.setConcurrencySelection;

	return (
		<div class="concurrency-control" title="Concurrent Jobs">
			<label class="concurrency-label" for="concurrency-select">
				Number of Jobs:
			</label>
			<select
				id="concurrency-select"
				class="concurrency-select"
				value={view().selection}
				disabled={!view().controlsEnabled}
				onChange={async (event) => {
					const select = event.currentTarget;
					await setSelection(select.value);
					select.value = view().selection;
				}}
			>
				<option value="auto">{`Auto · ${view().autoEffective} jobs`}</option>
				<For each={[...view().fixedOptions]}>
					{(option) => <option value={String(option)}>{option}</option>}
				</For>
			</select>
			<span class="sr-only" aria-live="polite">
				{view().effectiveLabel}
			</span>
		</div>
	);
}
