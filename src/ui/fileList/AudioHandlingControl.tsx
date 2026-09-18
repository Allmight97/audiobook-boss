import { Show, createEffect, createSignal } from 'solid-js';
import type { JSX } from '@solidjs/web';

import type { InputOwner } from '../../app/inputSession';
import { displayedTitleForFile } from '../../app/inputSession';
import type { AudioFile } from '../../types/audio';

const PRESERVATION_GUIDANCE =
	'This audiobook may not need re-encoding. Its audio is already compact. Keep it while updating tags, artwork, and its library folder.';

export function AudioHandlingControl(props: {
	readonly file: AudioFile;
	readonly index: number;
	readonly orderLocked: boolean;
	readonly setAudioHandling: InputOwner['setAudioHandling'];
}): JSX.Element {
	const [open, setOpen] = createSignal(false);
	const [pinned, setPinned] = createSignal(false);

	createEffect(open, (isOpen) => {
		if (!isOpen) return;
		function dismiss(event: KeyboardEvent): void {
			if (event.key !== 'Escape') return;
			event.preventDefault();
			event.stopPropagation();
			setPinned(false);
			setOpen(false);
		}
		document.addEventListener('keydown', dismiss, true);
		return () => document.removeEventListener('keydown', dismiss, true);
	});

	function guidanceId(): string {
		return `preservation-guidance-${props.index}`;
	}

	function closeIfUnpinned(): void {
		if (!pinned()) setOpen(false);
	}

	function handleFocusOut(event: FocusEvent): void {
		const nextTarget = event.relatedTarget;
		if (nextTarget instanceof Node && event.currentTarget instanceof HTMLElement) {
			if (event.currentTarget.contains(nextTarget)) return;
		}
		closeIfUnpinned();
	}

	function togglePinned(): void {
		if (pinned()) {
			setPinned(false);
			setOpen(false);
			return;
		}
		setPinned(true);
		setOpen(true);
	}

	return (
		// biome-ignore lint/a11y/noStaticElementInteractions: owns hover and focus state for its disclosure button
		<div
			class="preservation-info"
			onMouseEnter={() => setOpen(true)}
			onMouseLeave={closeIfUnpinned}
			onFocusIn={() => setOpen(true)}
			onFocusOut={handleFocusOut}
		>
			<button
				type="button"
				class="preservation-info-trigger"
				aria-label={`Why keep original audio for ${displayedTitleForFile(props.file)}`}
				aria-expanded={open() ? 'true' : 'false'}
				aria-controls={guidanceId()}
				onClick={(event) => {
					event.stopPropagation();
					togglePinned();
				}}
			>
				i
			</button>
			<Show when={open()}>
				<div id={guidanceId()} class="preservation-guidance" role="note">
					{PRESERVATION_GUIDANCE}
					<button
						type="button"
						class="preservation-guidance-action"
						disabled={props.orderLocked}
						aria-label={`Keep original audio for ${displayedTitleForFile(props.file)}`}
						onClick={(event) => {
							event.stopPropagation();
							props.setAudioHandling(props.file, 'preserve');
						}}
					>
						Keep original audio
					</button>
				</div>
			</Show>
		</div>
	);
}
