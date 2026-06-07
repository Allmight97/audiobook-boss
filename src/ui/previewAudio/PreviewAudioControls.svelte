<script lang="ts">
	import { startPreviewAudio } from '../core/actions';

	let { variant = 'default' }: { variant?: 'default' | 'compact' } = $props();

	let previewDuration = $state(30);
	let previewDropdownOpen = $state(false);
	let previewDropdownElement = $state<HTMLDivElement | null>(null);
	let previewDropdownToggleElement = $state<HTMLButtonElement | null>(null);

	function handlePreviewButtonClick(): void {
		startPreviewAudio(previewDuration);
	}

	function handlePreviewDropdownToggle(event: MouseEvent): void {
		event.stopPropagation();
		previewDropdownOpen = !previewDropdownOpen;
	}

	function handlePreviewDurationSelect(duration: number): void {
		previewDuration = duration;
		previewDropdownOpen = false;
		startPreviewAudio(duration);
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
	.split-button {
		position: relative;
		display: inline-flex;
	}

	.split-button-compact {
		flex-shrink: 0;
		max-width: 100%;
	}

	.split-main {
		border-top-right-radius: 0;
		border-bottom-right-radius: 0;
		border-right: none;
	}

	.split-button-compact .split-main {
		min-width: 0;
		padding: 0.35rem 0.6rem;
		overflow: hidden;
		font-size: 0.7rem;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.split-caret {
		padding: 0.25rem 0.35rem;
		border-top-left-radius: 0;
		border-bottom-left-radius: 0;
		font-size: 0.6rem;
	}

	.split-button-compact .split-caret {
		padding: 0.2rem 0.275rem;
	}

	.split-dropdown {
		position: absolute;
		top: 100%;
		right: 0;
		z-index: 20;
		display: none;
		min-width: 6.25rem;
		margin-top: 2px;
		overflow: hidden;
		border: 1px solid var(--border-secondary);
		border-radius: 0.375rem;
		background: var(--bg-panel);
		box-shadow: var(--shadow-md);
	}

	.split-dropdown.open {
		display: block;
	}

	.split-option {
		display: block;
		width: 100%;
		padding: 0.35rem 0.5rem;
		border: none;
		background: none;
		color: var(--text-secondary);
		font-size: 0.75rem;
		text-align: left;
	}

	.split-option:hover {
		background: var(--bg-input);
		color: var(--text-primary);
	}
</style>
