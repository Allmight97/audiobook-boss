<script lang="ts">
	import { startPreviewAudio } from '../core/actions';

	let previewDuration = 30;
	let previewDropdownOpen = false;
	let previewDropdownElement: HTMLDivElement | null = null;
	let previewDropdownToggleElement: HTMLButtonElement | null = null;

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

<svelte:window on:click={handleWindowClick} />

<div class="split-button">
	<button id="preview-button" class="btn-pill btn-pill-primary split-main" on:click={handlePreviewButtonClick}>
		Preview Audio
	</button>
	<button
		id="preview-dropdown-toggle"
		class="btn-pill btn-pill-primary split-caret"
		bind:this={previewDropdownToggleElement}
		on:click={handlePreviewDropdownToggle}
	>
		▼
	</button>
	<div
		id="preview-dropdown"
		class="split-dropdown"
		class:open={previewDropdownOpen}
		bind:this={previewDropdownElement}
	>
		<button class="split-option" data-duration="15" on:click={() => handlePreviewDurationSelect(15)}>
			15 seconds
		</button>
		<button class="split-option" data-duration="30" on:click={() => handlePreviewDurationSelect(30)}>
			30 seconds
		</button>
		<button class="split-option" data-duration="45" on:click={() => handlePreviewDurationSelect(45)}>
			45 seconds
		</button>
		<button class="split-option" data-duration="60" on:click={() => handlePreviewDurationSelect(60)}>
			60 seconds
		</button>
	</div>
</div>

<style>
	.split-button {
		position: relative;
		display: inline-flex;
	}

	.split-main {
		border-top-right-radius: 0;
		border-bottom-right-radius: 0;
		border-right: none;
	}

	.split-caret {
		padding: 0.25rem 0.35rem;
		border-top-left-radius: 0;
		border-bottom-left-radius: 0;
		font-size: 0.6rem;
	}

	.split-dropdown {
		position: absolute;
		top: 100%;
		right: 0;
		z-index: 10;
		display: none;
		min-width: 100px;
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
