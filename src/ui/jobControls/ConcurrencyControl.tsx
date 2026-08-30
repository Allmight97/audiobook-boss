import { For, type JSX } from 'solid-js';
import { useAppRuntime } from '../../app/runtime';

export function ConcurrencyControl(): JSX.Element {
	const settings = useAppRuntime().settings;
	const view = settings.concurrency;
	const setSelection = settings.setConcurrencySelection;

	return (
		<div class="flex items-center gap-1" title="Concurrent Jobs">
			<span class="text-xs muted-text whitespace-nowrap">Number of Jobs:</span>
			<select
				id="max-concurrent-select"
				class="text-xs w-14 px-1 py-0.5"
				style={{
					height: '24px',
					opacity: view().controlsEnabled ? 1 : 0.5,
				}}
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
				class="text-xs muted-text"
				aria-live="polite"
				data-testid="max-concurrent-effective"
			>
				{view().effectiveLabel}
			</span>
		</div>
	);
}
