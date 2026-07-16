<script lang="ts">
	import { triggerProcessFromStatusPanel } from '../statusPanel';

	const PREVIEW_DURATIONS = [15, 30, 45, 60];

	let menuOpen = $state(false);
	let rootElement = $state<HTMLElement | null>(null);
	let caretElement = $state<HTMLButtonElement | null>(null);

	function handleWindowClick(event: MouseEvent): void {
		if (!menuOpen) return;
		const target = event.target;
		if (!(target instanceof Node)) return;
		if (rootElement?.contains(target)) return;
		menuOpen = false;
	}

	function handleKeydown(event: KeyboardEvent): void {
		if (event.key === 'Escape' && menuOpen) {
			event.preventDefault();
			menuOpen = false;
			caretElement?.focus();
		}
	}

	function handlePreviewSelect(previewSeconds: number): void {
		menuOpen = false;
		triggerProcessFromStatusPanel({ previewSeconds });
	}
</script>

<svelte:window onclick={handleWindowClick} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="split-button" bind:this={rootElement} onkeydown={handleKeydown}>
	<button
		id="process-button"
		type="button"
		class="pill pill-primary split-main"
		onclick={() => triggerProcessFromStatusPanel()}
	>
		Process
	</button>
	<button
		id="process-menu-toggle"
		type="button"
		class="pill pill-primary split-caret"
		aria-haspopup="menu"
		aria-expanded={menuOpen}
		aria-label="Preview options"
		bind:this={caretElement}
		onclick={() => {
			menuOpen = !menuOpen;
		}}
	>
		▼
	</button>
	<div class="split-dropdown" class:open={menuOpen} role="menu">
		{#each PREVIEW_DURATIONS as seconds (seconds)}
			<button
				type="button"
				class="split-option"
				role="menuitem"
				onclick={() => handlePreviewSelect(seconds)}
			>
				Preview {seconds}s
			</button>
		{/each}
	</div>
</div>
