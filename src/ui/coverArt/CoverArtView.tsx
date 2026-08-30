import { onCleanup, onMount, type JSX } from 'solid-js';
import type { CoverArtMessage } from '../../app/metadataSession';
import { useAppRuntime } from '../../app/runtime';
import './coverArt.css';

function coverMessageText(message: CoverArtMessage): string {
	return message.kind === 'hidden' ? '' : message.text;
}

function isTextInput(target: EventTarget | null): boolean {
	if (!(target instanceof HTMLElement)) return false;
	const tagName = target.tagName.toLowerCase();
	return tagName === 'input' || tagName === 'textarea';
}

export function CoverArtView(): JSX.Element {
	const metadata = useAppRuntime().metadata;
	const view = metadata.view;
	const setHovered = metadata.setCoverHovered;
	const setDragOver = metadata.setCoverDragOver;
	const setUrlInput = metadata.setCoverUrlInput;
	const loadFromPicker = metadata.loadCoverArtFromPicker;
	const loadFromUrl = metadata.loadCoverArtFromUrl;
	const clearCover = metadata.clearCoverArt;

	function handleAreaKeyDown(event: KeyboardEvent): void {
		if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			void loadFromPicker();
		}
	}

	function handleWindowPaste(event: ClipboardEvent): void {
		const target = event.target;
		if (
			isTextInput(target) &&
			target instanceof HTMLElement &&
			target.id !== 'cover-art-url-input'
		) {
			return;
		}
		if (!view().cover.isHovered) return;
		const raw = event.clipboardData?.getData('text')?.trim();
		if (!raw) return;
		try {
			const pastedUrl = new URL(raw).toString();
			event.preventDefault();
			setUrlInput(pastedUrl);
			void loadFromUrl(pastedUrl);
		} catch {
			return;
		}
	}

	onMount(() => {
		window.addEventListener('paste', handleWindowPaste);
		onCleanup(() => window.removeEventListener('paste', handleWindowPaste));
	});

	const cover = () => view().cover;
	const hasImage = () => Boolean(cover().imageDataUrl);

	return (
		<div class="col-span-1 flex flex-col items-center">
			<span class="text-xs font-medium mb-1">Cover Art</span>
			{/* biome-ignore lint/a11y/useSemanticElements: cover drop target plus nested clear control */}
			<div
				id="cover-art-area"
				class="cover-art-area mb-1"
				classList={{
					'has-image': hasImage(),
					loading: cover().isLoading,
					'drag-over': cover().isDragOver,
				}}
				data-testid="cover-art-area"
				role="button"
				tabIndex={0}
				onClick={() => void loadFromPicker()}
				onKeyDown={handleAreaKeyDown}
				onMouseEnter={() => setHovered(true)}
				onMouseLeave={() => {
					setHovered(false);
					setDragOver(false);
				}}
				onDragOver={(event) => {
					event.preventDefault();
					setDragOver(true);
				}}
				onDragLeave={(event) => {
					event.preventDefault();
					setDragOver(false);
				}}
			>
				{!hasImage() && <div class="placeholder-text">Click or Drag Image</div>}
				<img
					src={cover().imageDataUrl ?? ''}
					alt="Book Cover Art"
					id="cover-art-img"
					classList={{ hidden: !hasImage() }}
				/>
				<div class="cover-art-loading" id="cover-art-loading">
					Loading...
				</div>
				<button
					type="button"
					class="cover-art-clear-btn"
					id="cover-art-clear-btn"
					aria-label="Clear cover art"
					tabIndex={hasImage() ? 0 : -1}
					onClick={(event) => {
						event.stopPropagation();
						clearCover();
					}}
				>
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
					value={cover().urlInputValue}
					disabled={cover().isLoading}
					onInput={(event) => setUrlInput(event.currentTarget.value)}
					onKeyDown={(event) => {
						if (event.key !== 'Enter') return;
						event.preventDefault();
						void loadFromUrl(cover().urlInputValue);
					}}
				/>
				<button
					id="cover-art-url-load-btn"
					data-testid="cover-art-url-load-btn"
					class="cover-art-url-load-btn"
					type="button"
					disabled={cover().isLoading}
					onClick={() => void loadFromUrl(cover().urlInputValue)}
				>
					Load
				</button>
			</div>
			<div
				id="cover-art-url-message"
				class="cover-art-url-message"
				classList={{
					visible: cover().message.kind !== 'hidden',
					'is-error': cover().message.kind === 'error',
					'is-success': cover().message.kind === 'success',
				}}
				role="status"
				aria-live="polite"
			>
				{coverMessageText(cover().message)}
			</div>
		</div>
	);
}
