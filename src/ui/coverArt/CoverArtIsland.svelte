<script lang="ts">
	type HoverChangeHandler = (isHovered: boolean) => void;

	export let onLoadFromFile: () => void;
	export let onClearCoverArt: () => void;
	export let onHoverChange: HoverChangeHandler;

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
		onHoverChange?.(true);
	}

	function handleMouseLeave(): void {
		onHoverChange?.(false);
	}

	function handleDragOver(event: DragEvent): void {
		event.preventDefault();
		const area = event.currentTarget as HTMLElement | null;
		area?.classList.add('drag-over');
	}

	function handleDragLeave(event: DragEvent): void {
		event.preventDefault();
		const area = event.currentTarget as HTMLElement | null;
		area?.classList.remove('drag-over');
	}
</script>

<div class="col-span-1 flex flex-col items-center">
  <span class="text-xs font-medium mb-1">Cover Art</span>
  <div
    id="cover-art-area"
    class="cover-art-area mb-1"
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
    <div class="placeholder-text">Click or Drag Image</div>
    <img src="" alt="Book Cover Art" id="cover-art-img" class="hidden" />
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
    />
    <button id="cover-art-url-load-btn" data-testid="cover-art-url-load-btn" class="cover-art-url-load-btn">
      Load
    </button>
  </div>
  <div id="cover-art-url-message" class="cover-art-url-message" role="status" aria-live="polite"></div>
</div>
