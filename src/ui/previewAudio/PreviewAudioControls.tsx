import { createSignal, onSettled } from 'solid-js';
import type { JSX } from '@solidjs/web';

import { useAppRuntime } from '../../app/runtime';
import { Button } from '../foundation';
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

	onSettled(() => {
		function handleWindowClick(event: MouseEvent): void {
			if (!open()) return;
			const target = event.target;
			if (!(target instanceof Node)) return;
			if (dropdown?.contains(target) || toggle?.contains(target)) return;
			setOpen(false);
		}
		window.addEventListener('click', handleWindowClick);
		return () => window.removeEventListener('click', handleWindowClick);
	});

	return (
		<div
			class={`split-button${props.variant === 'compact' ? ' split-button-compact' : ''}`}
			data-testid="preview-audio-controls"
		>
			<Button
				id="preview-button"
				tone="primary"
				class="split-main"
				onClick={() => void startProcessing({ previewSeconds: previewDuration() })}
			>
				Preview Audio
			</Button>
			<Button
				id="preview-dropdown-toggle"
				tone="primary"
				class="split-caret"
				ref={toggle}
				onClick={(event) => {
					event.stopPropagation();
					setOpen(!open());
				}}
			>
				▼
			</Button>
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
