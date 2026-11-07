import { For } from 'solid-js';
import type { JSX } from '@solidjs/web';

import { useAppRuntime } from '../../app/runtime';
import { Select } from '../foundation';
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
			<Select
				id="concurrency-select"
				class="concurrency-select"
				value={view().selection}
				disabled={!view().controlsEnabled}
				onChoose={setSelection}
			>
				<option value={view().autoSelection}>{`Auto · ${view().autoEffective} jobs`}</option>
				<For each={view().fixedOptions} keyed={(option) => option}>
					{(option) => <option value={String(option())}>{option()}</option>}
				</For>
			</Select>
			<span class="sr-only" aria-live="polite">
				{view().label}
			</span>
		</div>
	);
}
