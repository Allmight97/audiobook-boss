import { Show, createEffect, createSignal, onCleanup } from 'solid-js';
import { Portal, type JSX } from '@solidjs/web';
import { chapterPlansForProcessing, displayedTitleForFile } from '../../app/inputSession';
import { useAppRuntime } from '../../app/runtime';
import { EncoderView } from '../encoderPanel';
import { tauriClient } from '../../lib/tauri/client';
import { toUserMessage } from '../../lib/tauri/appError';
import type { AudioFile, TitleAudioPlan } from '../../types/audio';

export function AudioHandlingControl(props: {
	readonly file: AudioFile;
	readonly index: number;
	readonly orderLocked: boolean;
}): JSX.Element {
	const runtime = useAppRuntime();
	const [open, setOpen] = createSignal(false);
	const [plan, setPlan] = createSignal<TitleAudioPlan | undefined>(undefined);
	const [error, setError] = createSignal('');
	const [position, setPosition] = createSignal({ left: 0, top: 0 });
	let trigger: HTMLButtonElement | undefined;
	let panel: HTMLDivElement | undefined;
	let closeTimer: ReturnType<typeof setTimeout> | undefined;
	let pinned = false,
		dismissed = false,
		generation = 0,
		cachedKey = '';
	const request = () => runtime.encoding.audioRequest(props.file);
	const sources = () => runtime.input.sourcesFor(props.file);
	async function queryPlan() {
		const files = sources();
		return tauriClient.previewTitleAudio(
			files.map((file) => file.path),
			request(),
			chapterPlansForProcessing(files, files.length > 1 ? 'merge' : 'batch'),
		);
	}
	const needsChoice = () => runtime.input.audioChoiceRequired(props.file);
	const kept = () => plan()?.handling === 'preserve';
	const guidanceId = () => `audio-plan-${props.index}`;
	const format = () =>
		({ m4b: 'M4B', mp3: 'MP3', m4aOpus: 'M4A', mkaOpus: 'MKA' })[request().format];
	const opus = () => request().format === 'm4aOpus' || request().format === 'mkaOpus';
	function show() {
		if (dismissed) return;
		clearTimeout(closeTimer);
		setOpen(true);
	}
	function close() {
		clearTimeout(closeTimer);
		pinned = false;
		setOpen(false);
	}
	function leave() {
		clearTimeout(closeTimer);
		if (!pinned)
			closeTimer = setTimeout(() => {
				if (!panel?.contains(document.activeElement)) close();
			}, 180);
	}
	function dismiss() {
		dismissed = true;
		trigger?.focus();
		close();
	}
	createEffect(
		() => props.orderLocked,
		(locked) => {
			if (locked) close();
		},
	);
	onCleanup(() => {
		generation++;
		clearTimeout(closeTimer);
	});
	createEffect(
		() => ({
			open: open(),
			key: JSON.stringify({
				sources: sources().map((f) => ({
					path: f.path,
					chapterPlan: f.chapterPlan,
					cueSource: f.cueSource,
				})),
				request: request(),
				needsChoice: needsChoice(),
			}),
		}),
		({ open: isOpen, key }) => {
			if (key !== cachedKey) {
				generation++;
				cachedKey = '';
				setPlan(undefined);
				setError('');
			}
			if (!isOpen || needsChoice() || key === cachedKey) return;
			cachedKey = key;
			const ticket = ++generation;
			void queryPlan()
				.then((value) => {
					if (ticket === generation) setPlan(value);
				})
				.catch((err) => {
					if (ticket === generation) {
						cachedKey = '';
						setError(toUserMessage(err));
					}
				});
		},
	);
	createEffect(
		() => [open(), plan(), error()] as const,
		([isOpen]) => {
			if (!isOpen) return;
			function place() {
				const rect = trigger?.getBoundingClientRect();
				if (!rect) return;
				const width = Math.min(430, window.innerWidth - 24),
					height = panel?.offsetHeight ?? 300;
				setPosition({
					left: Math.max(12, Math.min(rect.left, window.innerWidth - width - 12)),
					top: Math.max(
						12,
						rect.bottom + height + 12 < window.innerHeight
							? rect.bottom + 8
							: rect.top - height - 8,
					),
				});
			}
			function onEscape(event: KeyboardEvent) {
				if (event.key !== 'Escape') return;
				event.preventDefault();
				event.stopPropagation();
				dismiss();
			}
			function outside(event: PointerEvent) {
				if (
					event.target instanceof Node &&
					!panel?.contains(event.target) &&
					!trigger?.contains(event.target)
				)
					close();
			}
			const resize = typeof ResizeObserver === 'undefined' ? undefined : new ResizeObserver(place);
			const frame = requestAnimationFrame(() => {
				if (panel) resize?.observe(panel);
				place();
			});
			place();
			document.addEventListener('keydown', onEscape, true);
			document.addEventListener('pointerdown', outside, true);
			window.addEventListener('resize', place);
			window.addEventListener('scroll', place, true);
			return () => {
				cancelAnimationFrame(frame);
				resize?.disconnect();
				document.removeEventListener('keydown', onEscape, true);
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
				ref={(el) => {
					trigger = el;
				}}
				class={[
					'preservation-info-trigger',
					{ kept: kept(), 'needs-choice': needsChoice() || !!error() },
				]}
				aria-label={`Audio plan for ${displayedTitleForFile(props.file)}`}
				aria-haspopup="dialog"
				aria-expanded={open() ? 'true' : 'false'}
				aria-controls={guidanceId()}
				disabled={props.orderLocked}
				onMouseEnter={show}
				onMouseLeave={() => {
					dismissed = false;
					leave();
				}}
				onFocus={show}
				onBlur={() => {
					dismissed = false;
				}}
				onClick={(event) => {
					event.stopPropagation();
					pinned = true;
					dismissed = false;
					show();
				}}
				onKeyDown={(event) => {
					if (event.key === 'Tab' && !event.shiftKey && open()) {
						event.preventDefault();
						pinned = true;
						panel?.querySelector<HTMLButtonElement>('button')?.focus();
					}
				}}
			>
				{kept() ? '✓' : 'i'}
			</button>
			<span class="title-audio-summary">
				{needsChoice() || error()
					? 'Choose audio'
					: kept()
						? `Pass-through · ${format()}`
						: `${request().intent === 'preserve' ? 'Keep original audio' : request().intent === 'auto' ? 'Recommended' : opus() ? 'Opus' : 'AAC'} · ${format()}`}
			</span>
			<Show
				when={
					needsChoice() || error()
						? null
						: runtime.output.estimateTitleSizeText(props.file, plan() ?? undefined)
				}
			>
				{(size) => (
					<span class="title-output-size" title="Estimated output size">
						{size()}
					</span>
				)}
			</Show>
			<Show when={open()}>
				<Portal>
					<div
						ref={(el) => {
							panel = el;
						}}
						id={guidanceId()}
						role="dialog"
						aria-label="Audio plan"
						class={[
							'preservation-guidance',
							'title-audio-plan',
							{ kept: kept(), 'needs-choice': needsChoice() || !!error() },
						]}
						style={{ left: `${position().left}px`, top: `${position().top}px` }}
						onMouseEnter={show}
						onMouseLeave={leave}
						onClick={(event) => event.stopPropagation()}
						onKeyDown={(event) => event.stopPropagation()}
						onPointerDown={(event) => {
							event.stopPropagation();
							pinned = true;
						}}
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
							aria-label="Close audio plan"
							onClick={dismiss}
						>
							×
						</button>
						<strong>
							{needsChoice()
								? 'Choose this title’s audio'
								: error()
									? 'Choose audio settings'
									: !plan()
										? 'Audio plan'
										: kept()
											? 'Keeping original audio'
											: `Using ${opus() ? 'Opus' : 'AAC'} with ${format()}`}
						</strong>
						<p>
							{needsChoice()
								? 'These sources had different audio settings. Choose settings for this title or apply App Settings.'
								: error() ||
									(!plan()
										? 'Checking source audio…'
										: kept()
											? 'Audio stays unchanged. Tags, artwork and chapters will be updated.'
											: `Convert ${plan()?.sourceCodec} to ${opus() ? 'Opus' : 'AAC'} and save as ${format()}.`)}
						</p>
						<p class="audio-plan-origin">
							<button
								type="button"
								onClick={() => runtime.encoding.applyDefaultsToTitles([props.file])}
							>
								Apply App Settings
							</button>
						</p>
						<EncoderView title={props.file} />
						<Show when={plan()}>
							{(resolved) => (
								<dl class="audio-plan-properties">
									<dt>Source audio</dt>
									<dd>{resolved().handling === 'preserve' ? 'Pass-through' : 'Re-encoded'}</dd>
									<dt>{opus() && !kept() ? 'Opus input rate' : 'Sample rate'}</dt>
									<dd>
										{sources()[0]?.sampleRate !== resolved().sampleRate
											? `${(sources()[0]?.sampleRate ?? 0) / 1000} → `
											: ''}
										{resolved().sampleRate / 1000} kHz
									</dd>
									<dt>Channels</dt>
									<dd>
										{resolved().channels === 1
											? 'Mono'
											: resolved().channels === 2
												? 'Stereo'
												: `${resolved().channels} channels`}
									</dd>
								</dl>
							)}
						</Show>
						<Show when={plan()?.reason}>
							<p>{plan()?.reason}</p>
						</Show>
						<Show when={opus() && !kept()}>
							<p class="audio-plan-origin">Opus playback uses a 48 kHz clock.</p>
						</Show>
						<Show when={opus()}>
							<p>
								{request().format === 'm4aOpus'
									? 'For Opus in Audiobookshelf, try M4A first. Confirm direct playback and chapters on your device.'
									: 'Use with VLC or another Matroska-capable player. Apple and browser clients may need transcoding.'}{' '}
								<button
									type="button"
									class="audio-compatibility-link"
									onClick={() =>
										void tauriClient.openUrl(
											request().format === 'mkaOpus'
												? 'https://www.videolan.org/vlc/features.html'
												: 'https://www.audiobookshelf.org/docs/faq/server/',
										)
									}
								>
									Player compatibility
								</button>
							</p>
						</Show>
					</div>
				</Portal>
			</Show>
		</>
	);
}
