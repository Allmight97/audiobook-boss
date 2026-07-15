<script lang="ts">
	import { triggerProcessFromStatusPanel } from '../statusPanel';

	let { variant = 'default' }: { variant?: 'default' | 'compact' } = $props();

	let previewDuration = $state(30);
	let previewDropdownOpen = $state(false);
	let previewDropdownElement = $state<HTMLDivElement | null>(null);
	let previewDropdownToggleElement = $state<HTMLButtonElement | null>(null);

	function handlePreviewButtonClick(): void {
		triggerProcessFromStatusPanel({ previewSeconds: previewDuration });
	}

	function handlePreviewDropdownToggle(event: MouseEvent): void {
		event.stopPropagation();
		previewDropdownOpen = !previewDropdownOpen;
	}

	function handlePreviewDurationSelect(duration: number): void {
		previewDuration = duration;
		previewDropdownOpen = false;
		triggerProcessFromStatusPanel({ previewSeconds: duration });
	}

	function handleWindowClick(event: MouseEvent): void {
		if (!previewDropdownOpen) return;
		const target = event.target;
		if (!(target instanceof Node)) return;
		if (
			previewDropdownElement?.contains(target) ||
			previewDropdownToggleElement?.contains(target)
		) {
			return;
		}
		previewDropdownOpen = false;
	}
</script>

<svelte:window onclick={handleWindowClick} />

<div class="split-button" class:split-button-compact={variant === 'compact'} data-testid="preview-audio-controls">
	<button id="preview-button" class="btn-pill btn-pill-primary split-main" onclick={handlePreviewButtonClick}>
		Preview Audio
	</button>
	<button
		id="preview-dropdown-toggle"
		class="btn-pill btn-pill-primary split-caret"
		bind:this={previewDropdownToggleElement}
		onclick={handlePreviewDropdownToggle}
	>
		▼
	</button>
	<div
		id="preview-dropdown"
		class="split-dropdown"
		class:open={previewDropdownOpen}
		bind:this={previewDropdownElement}
	>
		<button class="split-option" data-duration="15" onclick={() => handlePreviewDurationSelect(15)}>
			15 seconds
		</button>
		<button class="split-option" data-duration="30" onclick={() => handlePreviewDurationSelect(30)}>
			30 seconds
		</button>
		<button class="split-option" data-duration="45" onclick={() => handlePreviewDurationSelect(45)}>
			45 seconds
		</button>
		<button class="split-option" data-duration="60" onclick={() => handlePreviewDurationSelect(60)}>
			60 seconds
		</button>
	</div>
</div>

<style>
	/* Split-button layout/dropdown primitives live in src/styles.css; only the
	   compact variant (one owner) stays local. */
	.split-button-compact {
		flex-shrink: 0;
		max-width: 100%;
	}

	.split-button-compact :global(.split-main) {
		min-width: 0;
		padding: 0.35rem 0.6rem;
		overflow: hidden;
		font-size: 0.7rem;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.split-button-compact :global(.split-caret) {
		padding: 0.2rem 0.275rem;
	}
</style>
