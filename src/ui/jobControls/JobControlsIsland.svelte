<script lang="ts">
	import { onMount } from 'svelte';
	import { initJobControls } from '../jobControls';
	import { jobControlsState } from './state.svelte';

	interface Props {
		fileCount?: number;
		onMergeModeChange?: (checked: boolean) => void;
	}

	let { fileCount = 0, onMergeModeChange = () => {} }: Props = $props();

	const isMerge = $derived(jobControlsState.jobType === 'merge');
	const chipLabel = $derived(
		isMerge ? `Merge — ${fileCount} ${fileCount === 1 ? 'file' : 'files'} → one M4B` : 'Merge off',
	);

	onMount(() => {
		initJobControls();
	});
</script>

<button
	type="button"
	id="merge-mode-toggle"
	data-testid="merge-toggle"
	class="merge-chip app-badge {isMerge ? 'app-badge-info' : 'app-badge-muted'}"
	aria-pressed={isMerge}
	disabled={!jobControlsState.controlsEnabled}
	style:opacity={jobControlsState.controlsEnabled ? 1 : 0.5}
	title="Combines all files in the list into a single audiobook in the order they appear. Each file will be treated as a chapter."
	onclick={() => onMergeModeChange(!isMerge)}
>
	{chipLabel}
</button>

<style>
	/* Button reset so the app-badge chip look owns the visuals (one owner — stays local). */
	.merge-chip {
		margin-top: 0;
		border: none;
		cursor: pointer;
		transition: all var(--dur-base) var(--ease-out);
	}

	.merge-chip:disabled {
		cursor: not-allowed;
	}

	.merge-chip:not(:disabled):hover {
		filter: brightness(1.15);
	}
</style>
