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
			<span class="concurrency-label">Number of Jobs:</span>
			<select
				id="max-concurrent-select"
				class="concurrency-select"
				value={view().selection}
				disabled={!view().controlsEnabled}
				onChange={(event) => void setSelection(event.currentTarget.value)}
			>
				{view().allowAuto && <option value="auto">Auto</option>}
				<For each={[...view().fixedOptions]}>
					{(option) => <option value={String(option)}>{option}</option>}
				</For>
			</select>
			<span
				id="max-concurrent-effective"
				class="concurrency-effective"
				aria-live="polite"
				data-testid="max-concurrent-effective"
			>
				{view().effectiveLabel}
			</span>
		</div>
	);
}
