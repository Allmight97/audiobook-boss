import { createSignal, onCleanup, onMount, type JSX } from 'solid-js';
import { useAppRuntime } from '../../app/runtime';
import './previewAudioControls.css';

const PREVIEW_DURATIONS = [15, 30, 45, 60] as const;

export function PreviewAudioControls(props: {
	readonly variant?: 'default' | 'compact';
}): JSX.Element {
	const startProcessing = useAppRuntime().processing.start;
	const [previewDuration, setPreviewDuration] = createSignal(30);
	const [open, setOpen] = createSignal(false);
	let dropdown: HTMLDivElement | undefined;
	let toggle: HTMLButtonElement | undefined;

	onMount(() => {
		function handleWindowClick(event: MouseEvent): void {
			if (!open()) return;
			const target = event.target;
			if (!(target instanceof Node)) return;
			if (dropdown?.contains(target) || toggle?.contains(target)) return;
			setOpen(false);
		}
		window.addEventListener('click', handleWindowClick);
		onCleanup(() => window.removeEventListener('click', handleWindowClick));
	});

	return (
		<div
			class={`split-button${props.variant === 'compact' ? ' split-button-compact' : ''}`}
			data-testid="preview-audio-controls"
		>
			<button
				id="preview-button"
				class="btn-pill btn-pill-primary split-main"
				type="button"
				onClick={() => void startProcessing({ previewSeconds: previewDuration() })}
			>
				Preview Audio
			</button>
			<button
				id="preview-dropdown-toggle"
				class="btn-pill btn-pill-primary split-caret"
				type="button"
				ref={toggle}
				onClick={(event) => {
					event.stopPropagation();
					setOpen(!open());
				}}
			>
				▼
			</button>
			<div id="preview-dropdown" class={`split-dropdown${open() ? ' open' : ''}`} ref={dropdown}>
				{PREVIEW_DURATIONS.map((duration) => (
					<button
						class="split-option"
						type="button"
						data-duration={String(duration)}
						onClick={() => {
							setPreviewDuration(duration);
							setOpen(false);
							void startProcessing({ previewSeconds: duration });
						}}
					>
						{duration} seconds
					</button>
				))}
			</div>
		</div>
	);
}
