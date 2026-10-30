import { createEffect, createSignal, onCleanup, Show } from 'solid-js';
import type { JSX } from '@solidjs/web';

import { ModalController } from './internal/modal';

export type DialogProps = {
	readonly open: boolean;
	readonly onClose: () => void;
	readonly labelledBy: string;
	readonly id?: string;
	readonly testId?: string;
	readonly restoreFocus?: boolean;
	/** Mount the content only while open or fading out, so a closed dialog does no work. */
	readonly unmountWhenClosed?: boolean;
	readonly children: JSX.Element;
};

export type DialogStatusTone = 'neutral' | 'error' | 'success';

function Header(props: { readonly class?: string; readonly children: JSX.Element }): JSX.Element {
	return (
		<div class={`abb-dialog-header${props.class ? ` ${props.class}` : ''}`}>{props.children}</div>
	);
}

function Body(props: { readonly class?: string; readonly children: JSX.Element }): JSX.Element {
	return (
		<div class={`abb-dialog-body${props.class ? ` ${props.class}` : ''}`}>{props.children}</div>
	);
}

function Status(props: {
	readonly tone?: DialogStatusTone;
	readonly class?: string;
	readonly id?: string;
	readonly live?: 'polite' | 'off';
	readonly children: JSX.Element;
}): JSX.Element {
	const tone = () => props.tone ?? 'neutral';
	return (
		<div
			id={props.id}
			aria-live={props.live}
			class={[
				'abb-dialog-status',
				props.class,
				{
					'is-error': tone() === 'error',
					'is-success': tone() === 'success',
				},
			]}
		>
			{props.children}
		</div>
	);
}

function DialogRoot(props: DialogProps): JSX.Element {
	let dialogEl: HTMLDivElement | undefined;
	let backdropEl: HTMLDivElement | undefined;
	const modal = new ModalController();
	const [faded, setFaded] = createSignal(true, { ownedWrite: true });
	let fadeTimer: ReturnType<typeof setTimeout> | undefined;
	const fadeMs = () =>
		Math.max(
			0,
			...(backdropEl ? getComputedStyle(backdropEl).transitionDuration : '')
				.split(',')
				.map((part) => Number.parseFloat(part) * 1000)
				.filter(Number.isFinite),
		);

	createEffect(
		() => ({
			open: props.open,
			onClose: props.onClose,
			restoreFocus: props.restoreFocus !== false,
		}),
		(value) => {
			modal.sync(
				value.open,
				{ container: dialogEl ?? null },
				{ onEscape: value.onClose, restoreInvoker: value.restoreFocus },
			);
			clearTimeout(fadeTimer);
			if (value.open) setFaded(false);
			else fadeTimer = setTimeout(() => setFaded(true), fadeMs());
		},
	);
	onCleanup(() => {
		clearTimeout(fadeTimer);
		modal.destroy();
	});

	return (
		<div
			id={props.id}
			class={['abb-dialog-backdrop', { open: props.open }]}
			data-testid={props.testId}
			aria-hidden={props.open ? undefined : 'true'}
			ref={(element) => {
				backdropEl = element;
			}}
			onClick={(event) => {
				if (event.target === event.currentTarget) {
					props.onClose();
				}
			}}
		>
			<div
				class="abb-dialog"
				role="dialog"
				aria-modal="true"
				aria-labelledby={props.labelledBy}
				ref={(element) => {
					dialogEl = element;
				}}
			>
				{props.unmountWhenClosed ? (
					<Show when={props.open || !faded()}>{props.children}</Show>
				) : (
					props.children
				)}
			</div>
		</div>
	);
}

export const Dialog = Object.assign(DialogRoot, { Header, Body, Status });
