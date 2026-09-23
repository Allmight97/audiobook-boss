import { Show, createEffect, createSignal, createUniqueId } from 'solid-js';
import { Portal, type JSX } from '@solidjs/web';
import { useAppRuntime } from '../../app/runtime';
import { EncoderView } from '../encoderPanel';
import './fileList.css';

/** Selection stays owned by Input; this view only owns the editor's disclosure. */
export function SelectedAudioSettings(): JSX.Element {
	const runtime = useAppRuntime();
	const selected = () => {
		const { files, selectedIndices } = runtime.input.view();
		return files.filter((_, index) => selectedIndices.includes(index));
	};
	const selectionKey = () => JSON.stringify(selected().map((file) => file.path));
	const [openedFor, setOpenedFor] = createSignal<string | undefined>(undefined);
	const [position, setPosition] = createSignal({ left: 12, top: 12 });
	const open = () =>
		openedFor() === selectionKey() && selected().length > 1 && !runtime.input.view().orderLocked;
	const id = createUniqueId();
	let trigger: HTMLButtonElement | undefined;
	let panel: HTMLDivElement | undefined;
	function close(restoreFocus = false) {
		setOpenedFor(undefined);
		if (restoreFocus) trigger?.focus();
	}
	createEffect(
		() => ({ key: selectionKey(), locked: runtime.input.view().orderLocked, opened: openedFor() }),
		({ key, locked, opened }) => {
			if (opened && (locked || opened !== key)) setOpenedFor(undefined);
		},
	);
	createEffect(open, (visible) => {
		if (!visible) return;
		function place() {
			const rect = trigger?.getBoundingClientRect();
			if (!rect) return;
			setPosition({
				left: Math.max(12, Math.min(rect.left, window.innerWidth - 442)),
				top: Math.max(
					12,
					Math.min(rect.bottom + 8, window.innerHeight - (panel?.offsetHeight ?? 300) - 12),
				),
			});
		}
		function outside(event: PointerEvent) {
			if (
				event.target instanceof Node &&
				!panel?.contains(event.target) &&
				!trigger?.contains(event.target)
			)
				close();
		}
		function onEscape(event: KeyboardEvent) {
			if (event.key !== 'Escape') return;
			event.preventDefault();
			event.stopPropagation();
			close(true);
		}
		const resize = typeof ResizeObserver === 'undefined' ? undefined : new ResizeObserver(place);
		const frame = requestAnimationFrame(() => {
			if (panel) resize?.observe(panel);
			place();
			panel?.querySelector<HTMLButtonElement>('button')?.focus();
		});
		place();
		document.addEventListener('pointerdown', outside, true);
		document.addEventListener('keydown', onEscape, true);
		window.addEventListener('resize', place);
		window.addEventListener('scroll', place, true);
		return () => {
			cancelAnimationFrame(frame);
			resize?.disconnect();
			document.removeEventListener('pointerdown', outside, true);
			document.removeEventListener('keydown', onEscape, true);
			window.removeEventListener('resize', place);
			window.removeEventListener('scroll', place, true);
		};
	});
	return (
		<Show when={selected().length > 1}>
			<button
				type="button"
				ref={trigger}
				class="group-titles-button"
				aria-haspopup="dialog"
				aria-expanded={open() ? 'true' : 'false'}
				aria-controls={id}
				disabled={runtime.input.view().orderLocked}
				onClick={() => (open() ? close() : setOpenedFor(selectionKey()))}
			>
				Audio settings · {selected().length} titles
			</button>
			<Show when={open()}>
				<Portal>
					<div
						ref={panel}
						id={id}
						role="dialog"
						aria-label={`Audio settings for ${selected().length} selected titles`}
						class="preservation-guidance title-audio-plan"
						style={{ left: `${position().left}px`, top: `${position().top}px` }}
						onFocusOut={(event) => {
							if (
								event.relatedTarget instanceof Node &&
								!panel?.contains(event.relatedTarget) &&
								!trigger?.contains(event.relatedTarget)
							)
								close();
						}}
					>
						<button
							type="button"
							class="audio-plan-close"
							aria-label="Close audio settings"
							onClick={() => close(true)}
						>
							×
						</button>
						<strong>Audio for {selected().length} selected titles</strong>
						<p>Changes apply to these titles. Other settings stay as they are.</p>
						<p class="audio-plan-origin">
							<button
								type="button"
								onClick={() => runtime.encoding.applyDefaultsToTitles(selected())}
							>
								Use defaults
							</button>
						</p>
						<EncoderView titles={selected()} />
					</div>
				</Portal>
			</Show>
		</Show>
	);
}
