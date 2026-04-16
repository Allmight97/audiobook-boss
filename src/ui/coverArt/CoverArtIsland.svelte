<script lang="ts">
	import {
		clearCoverArtMessageState,
		coverArtUiState,
		setCoverArtDragOver,
		setCoverArtHovered,
	} from './state.svelte';

	export let onLoadFromFile: () => void;
	export let onLoadFromInput: (raw: string) => Promise<string | null>;
	export let onClearCoverArt: () => void;

	async function loadFromInputValue(): Promise<void> {
		const normalized = await onLoadFromInput?.(coverArtUiState.urlInputValue ?? '');
		if (normalized) {
			coverArtUiState.urlInputValue = normalized;
		}
	}

	function handleAreaClick(): void {
		onLoadFromFile?.();
	}

	function handleAreaKeyDown(event: KeyboardEvent): void {
		if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			onLoadFromFile?.();
		}
	}

	function handleClearButtonClick(event: MouseEvent): void {
		event.stopPropagation();
		onClearCoverArt?.();
	}

	function handleMouseEnter(): void {
		setCoverArtHovered(true);
	}

	function handleMouseLeave(): void {
		setCoverArtHovered(false);
		setCoverArtDragOver(false);
	}

	function handleDragOver(event: DragEvent): void {
		event.preventDefault();
		setCoverArtDragOver(true);
	}

	function handleDragLeave(event: DragEvent): void {
		event.preventDefault();
		setCoverArtDragOver(false);
	}

	function handleUrlInputKeyDown(event: KeyboardEvent): void {
		if (event.key !== 'Enter') return;
		event.preventDefault();
		void loadFromInputValue();
	}

	async function handleUrlLoadClick(): Promise<void> {
		await loadFromInputValue();
	}

	function isTextInput(target: HTMLElement): boolean {
		const tagName = target.tagName.toLowerCase();
		return tagName === 'input' || tagName === 'textarea';
	}

	function getUrlFromClipboard(event: ClipboardEvent): string | null {
		const raw = event.clipboardData?.getData('text')?.trim();
		if (!raw) return null;
		try {
			return new URL(raw).toString();
		} catch {
			return null;
		}
	}

	function handleWindowPaste(event: ClipboardEvent): void {
		const target = event.target as HTMLElement | null;
		if (target && isTextInput(target) && target.id !== 'cover-art-url-input') {
			return;
		}
		if (!coverArtUiState.isHovered) return;
		const pastedUrl = getUrlFromClipboard(event);
		if (!pastedUrl) return;
		event.preventDefault();
		coverArtUiState.urlInputValue = pastedUrl;
		clearCoverArtMessageState();
		void loadFromInputValue();
	}
</script>

<svelte:window on:paste={handleWindowPaste} />

<div class="col-span-1 flex flex-col items-center">
  <span class="text-xs font-medium mb-1">Cover Art</span>
  <div
    id="cover-art-area"
    class="cover-art-area mb-1"
    class:has-image={Boolean(coverArtUiState.imageDataUrl)}
    class:loading={coverArtUiState.isLoading}
    class:drag-over={coverArtUiState.isDragOver}
    data-testid="cover-art-area"
    role="button"
    tabindex="0"
    on:click={handleAreaClick}
    on:keydown={handleAreaKeyDown}
    on:mouseenter={handleMouseEnter}
    on:mouseleave={handleMouseLeave}
    on:dragover={handleDragOver}
    on:dragleave={handleDragLeave}
  >
    {#if !coverArtUiState.imageDataUrl}
      <div class="placeholder-text">Click or Drag Image</div>
    {/if}
    <img
      src={coverArtUiState.imageDataUrl ?? ''}
      alt="Book Cover Art"
      id="cover-art-img"
      class:hidden={!coverArtUiState.imageDataUrl}
    />
    <div class="cover-art-loading" id="cover-art-loading">Loading...</div>
    <button class="cover-art-clear-btn" id="cover-art-clear-btn" on:click={handleClearButtonClick}>
      ✕
    </button>
  </div>
  <div class="cover-art-url-row">
    <input
      type="text"
      id="cover-art-url-input"
      data-testid="cover-art-url-input"
      class="cover-art-url-input"
      placeholder="Paste image URL (https://...)"
      bind:value={coverArtUiState.urlInputValue}
      disabled={coverArtUiState.isLoading}
      on:keydown={handleUrlInputKeyDown}
    />
    <button
      id="cover-art-url-load-btn"
      data-testid="cover-art-url-load-btn"
      class="cover-art-url-load-btn"
      disabled={coverArtUiState.isLoading}
      on:click={handleUrlLoadClick}
    >
      Load
    </button>
  </div>
  <div
    id="cover-art-url-message"
    class="cover-art-url-message"
    class:visible={coverArtUiState.message.kind !== 'hidden'}
    class:is-error={coverArtUiState.message.kind === 'error'}
    class:is-success={coverArtUiState.message.kind === 'success'}
    role="status"
    aria-live="polite"
  >
    {coverArtUiState.message.kind === 'hidden' ? '' : coverArtUiState.message.text}
  </div>
</div>
