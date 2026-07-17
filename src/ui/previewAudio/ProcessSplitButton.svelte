<script lang="ts">
	import { PopoverController } from '../../lib/ui/popover.svelte';
	import { triggerProcessFromStatusPanel } from '../statusPanel';

	const PREVIEW_DURATIONS = [15, 30, 45, 60];

	let rootElement = $state<HTMLElement | null>(null);
	let caretElement = $state<HTMLButtonElement | null>(null);
	let menuPanel = $state<HTMLElement | null>(null);
	const menu = new PopoverController({ closeOnClickAway: true });

	$effect(() => {
		menu.setElements({ anchor: caretElement, panel: menuPanel, clickBoundary: rootElement });
	});

	function handleWindowClick(event: MouseEvent): void {
		menu.handleClickAway(event);
	}

	function handleKeydown(event: KeyboardEvent): void {
		menu.handleKeydown(event);
	}

	function toggleFromKeyboard(event: KeyboardEvent): void {
		if (!['Enter', ' ', 'ArrowDown'].includes(event.key)) return;
		event.preventDefault();
		menu.toggle({ focusInside: true });
	}

	function handlePreviewSelect(previewSeconds: number): void {
		menu.close({ restoreFocus: false });
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
		aria-expanded={menu.isOpen}
		aria-label="Preview options"
		bind:this={caretElement}
		onclick={() => menu.toggle({ focusInside: false })}
		onkeydown={toggleFromKeyboard}
	>
		▼
	</button>
	<div bind:this={menuPanel} class="split-dropdown" class:open={menu.isOpen} role="menu">
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
