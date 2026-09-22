import { Show, createEffect, createSignal, onCleanup } from 'solid-js';
import { Portal, type JSX } from '@solidjs/web';
import type { InputOwner } from '../../app/inputSession';
import { displayedTitleForFile } from '../../app/inputSession';
import type { AudioFile, AudioHandling } from '../../types/audio';

const PRESERVATION_GUIDANCE =
	'This audio is already compact. Skip re-encoding while updating tags, artwork, and its library folder.';

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
	onCleanup(() => clearTimeout(closeTimer));
	createEffect(
		() => [open(), kept(), props.choiceRequired] as const,
		([isOpen]) => {
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
		},
	);
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
				aria-label={`${props.canPreserve ? 'Keep original audio' : 'Re-encode audio'} for ${displayedTitleForFile(props.file)}`}
				aria-pressed={props.choiceRequired ? 'mixed' : kept() ? 'true' : 'false'}
				aria-describedby={open() ? guidanceId() : undefined}
				disabled={props.orderLocked}
				onMouseEnter={show}
				onMouseLeave={leave}
				onFocus={show}
				onBlur={() => setOpen(false)}
				onClick={(event) => {
					event.stopPropagation();
					props.setAudioHandling(props.file, kept() || !props.canPreserve ? 'encode' : 'preserve');
					show();
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
						role="tooltip"
						class={[
							'preservation-guidance',
							{ kept: kept(), recommended: props.recommended || props.choiceRequired },
						]}
						style={{ left: `${position().left}px`, top: `${position().top}px` }}
						onMouseEnter={show}
						onMouseLeave={leave}
					>
						<strong>
							{props.choiceRequired
								? 'Choose audio handling'
								: kept()
									? 'Original audio kept'
									: 'Keep original audio'}
						</strong>
						<p>
							{props.choiceRequired
								? props.canPreserve
									? 'These files had different audio choices. Choose one setting for this title.'
									: 'These files cannot all keep their original audio. Re-encode this title to continue.'
								: kept()
									? 'Audio will be copied without re-encoding, with your updated tags, artwork, and library folder.'
									: props.recommended
										? PRESERVATION_GUIDANCE
										: 'Keep the source audio without re-encoding while updating tags, artwork, and its library folder.'}
						</p>
						<Show when={props.grouped && props.canPreserve}>
							<p class="muted-text">
								Keeping audio in a merged title requires compatible AAC sources.
							</p>
						</Show>
						<small>
							{kept() || !props.canPreserve
								? 'Click the indicator to re-encode.'
								: props.choiceRequired
									? 'Click to keep original audio; click again to re-encode.'
									: 'Click the indicator to keep original audio.'}
						</small>
					</div>
				</Portal>
			</Show>
		</>
	);
}
