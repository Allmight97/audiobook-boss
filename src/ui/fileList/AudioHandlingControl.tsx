import { Show, createEffect, createSignal, onCleanup } from 'solid-js';
import { Portal, type JSX } from '@solidjs/web';
import type { InputOwner } from '../../app/inputSession';
import { displayedTitleForFile } from '../../app/inputSession';
import type { AudioFile, AudioHandling } from '../../types/audio';

const PRESERVATION_GUIDANCE =
	'This audiobook may not need re-encoding. Its audio is already compact. Keep it while updating tags, artwork, and its library folder.';

export function AudioHandlingControl(props: {
	readonly file: AudioFile;
	readonly index: number;
	readonly orderLocked: boolean;
	readonly handling: AudioHandling;
	readonly choiceRequired: boolean;
	readonly canPreserve: boolean;
	readonly grouped: boolean;
	readonly recommended: boolean;
	readonly setAudioHandling: InputOwner['setAudioHandling'];
}): JSX.Element {
	const [open, setOpen] = createSignal(false);
	const [position, setPosition] = createSignal({ left: 0, top: 0 });
	let trigger: HTMLButtonElement | undefined;
	let panel: HTMLDivElement | undefined;
	let closeTimer: ReturnType<typeof setTimeout> | undefined;
	const kept = () => props.handling === 'preserve' && !props.choiceRequired;
	const guidanceId = () => `preservation-guidance-${props.index}`;
	function show(): void {
		clearTimeout(closeTimer);
		setOpen(true);
	}
	function leave(): void {
		clearTimeout(closeTimer);
		closeTimer = setTimeout(() => setOpen(false), 160);
	}
	function focusOut(event: FocusEvent): void {
		const target = event.relatedTarget;
		if (target instanceof Node && (panel?.contains(target) || trigger?.contains(target))) return;
		setOpen(false);
	}
	onCleanup(() => clearTimeout(closeTimer));
	createEffect(open, (isOpen) => {
		if (!isOpen) return;
		function place(): void {
			const rect = trigger?.getBoundingClientRect();
			if (!rect) return;
			const width = Math.min(360, window.innerWidth - 24);
			const height = panel?.offsetHeight ?? 220;
			setPosition({
				left: Math.max(12, Math.min(rect.left, window.innerWidth - width - 12)),
				top:
					rect.bottom + height + 12 < window.innerHeight
						? rect.bottom + 8
						: Math.max(12, rect.top - height - 8),
			});
		}
		function dismiss(event: KeyboardEvent): void {
			if (event.key !== 'Escape') return;
			event.preventDefault();
			event.stopPropagation();
			if (panel?.contains(document.activeElement)) trigger?.focus();
			setOpen(false);
		}
		function outside(event: PointerEvent): void {
			if (
				event.target instanceof Node &&
				!panel?.contains(event.target) &&
				!trigger?.contains(event.target)
			)
				setOpen(false);
		}
		const frame = requestAnimationFrame(place);
		place();
		document.addEventListener('keydown', dismiss, true);
		document.addEventListener('pointerdown', outside, true);
		window.addEventListener('resize', place);
		window.addEventListener('scroll', place, true);
		return () => {
			cancelAnimationFrame(frame);
			document.removeEventListener('keydown', dismiss, true);
			document.removeEventListener('pointerdown', outside, true);
			window.removeEventListener('resize', place);
			window.removeEventListener('scroll', place, true);
		};
	});
	return (
		<>
			<button
				type="button"
				ref={(element) => {
					trigger = element;
				}}
				class={[
					'preservation-info-trigger',
					{ kept: kept(), recommended: props.recommended || props.choiceRequired },
				]}
				aria-label={`${kept() ? 'Original audio kept' : 'Audio handling'} for ${displayedTitleForFile(props.file)}`}
				aria-expanded={open() ? 'true' : 'false'}
				aria-controls={guidanceId()}
				aria-haspopup="dialog"
				onMouseEnter={show}
				onMouseLeave={leave}
				onFocus={show}
				onFocusOut={focusOut}
				onClick={(event) => {
					event.stopPropagation();
					show();
					if (event.detail === 0)
						requestAnimationFrame(() => panel?.querySelector('button')?.focus());
				}}
			>
				{kept() ? '✓' : 'i'}
			</button>
			<Show when={open()}>
				<Portal>
					<div
						ref={(element) => {
							panel = element;
						}}
						id={guidanceId()}
						role="dialog"
						aria-label="Audio handling"
						class={[
							'preservation-guidance',
							{ kept: kept(), recommended: props.recommended || props.choiceRequired },
						]}
						style={{ left: `${position().left}px`, top: `${position().top}px` }}
						onMouseEnter={show}
						onMouseLeave={leave}
						onFocusIn={show}
						onFocusOut={focusOut}
					>
						<strong>
							{props.choiceRequired
								? 'Choose audio handling for this title'
								: kept()
									? 'Original audio kept'
									: 'Keep your original audio'}
						</strong>
						<p>
							{props.choiceRequired
								? 'These files had different audio choices. Choose how to process this title.'
								: kept()
									? 'The original audio will be copied without re-encoding when you process this title. You can still update tags, artwork, and its library folder.'
									: props.recommended
										? PRESERVATION_GUIDANCE
										: 'Keep the source audio without re-encoding while updating tags, artwork, and its library folder.'}
						</p>
						<Show when={props.grouped}>
							<p class="muted-text">
								Keeping audio requires compatible sources. Compatibility is checked before
								processing.
							</p>
						</Show>
						<button
							type="button"
							class="preservation-guidance-action"
							disabled={props.orderLocked || (!kept() && !props.canPreserve)}
							aria-pressed={kept() ? 'true' : 'false'}
							onClick={() => props.setAudioHandling(props.file, kept() ? 'encode' : 'preserve')}
						>
							{kept() ? '✓ Original audio kept' : 'Keep original audio'}
						</button>
						<Show when={kept()}>
							<small>Click again to re-encode.</small>
						</Show>
						<Show when={props.choiceRequired}>
							<button
								type="button"
								disabled={props.orderLocked}
								onClick={() => props.setAudioHandling(props.file, 'encode')}
							>
								Re-encode this title
							</button>
						</Show>
					</div>
				</Portal>
			</Show>
		</>
	);
}
