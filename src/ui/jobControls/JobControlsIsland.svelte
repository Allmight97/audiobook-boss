<script lang="ts">
	import { onMount } from 'svelte';
	import { initJobControls } from '../jobControls';
	import { jobControlsState } from './state.svelte';

	export let onMergeModeChange: (checked: boolean) => void = () => {};
	export let onMaxConcurrentSelectionChange: (value: string) => void = () => {};

	function handleMergeModeChange(event: Event): void {
		const input = event.currentTarget as HTMLInputElement | null;
		onMergeModeChange(Boolean(input?.checked));
	}

	function handleMaxConcurrentSelectionChange(event: Event): void {
		const select = event.currentTarget as HTMLSelectElement | null;
		onMaxConcurrentSelectionChange(select?.value ?? 'auto');
	}

	onMount(() => {
		initJobControls();
	});
</script>

<div class="flex items-center gap-2">
	<div class="flex items-center gap-1.5">
		<div class="relative group">
			<div class="info-icon">i</div>
			<div class="info-popover">
				Combines all files in the list into a single audiobook in the order they appear. Each file
				will be treated as a chapter.
			</div>
		</div>
		<label class="checkbox-label text-xs mb-0" data-testid="merge-toggle">
			<input
				type="checkbox"
				id="merge-mode-toggle"
				checked={jobControlsState.jobType === 'merge'}
				disabled={!jobControlsState.controlsEnabled}
				style:opacity={jobControlsState.controlsEnabled ? 1 : 0.5}
				onchange={handleMergeModeChange}
			/>
			<span class="option-label">Merge files into one audiobook</span>
		</label>
	</div>

	<div class="flex items-center gap-1" title="Concurrent Jobs">
		<span class="text-xs muted-text whitespace-nowrap">Number of Jobs:</span>
		<select
			id="max-concurrent-select"
			class="text-xs w-14 px-1 py-0.5"
			style:height={'24px'}
			style:opacity={jobControlsState.controlsEnabled ? 1 : 0.5}
			value={jobControlsState.maxConcurrentSelection}
			disabled={!jobControlsState.controlsEnabled || !jobControlsState.maxConcurrentCapabilities}
			onchange={handleMaxConcurrentSelectionChange}
		>
			{#if jobControlsState.maxConcurrentCapabilities?.allowAuto}
				<option value="auto">Auto</option>
			{/if}
			{#each jobControlsState.maxConcurrentCapabilities?.fixedOptions ?? [] as option}
				<option value={String(option)}>{option}</option>
			{/each}
		</select>
		<span
			id="max-concurrent-effective"
			class="text-xs muted-text"
			aria-live="polite"
			data-testid="max-concurrent-effective"
		>
			{jobControlsState.effectiveLabel}
		</span>
	</div>
</div>
