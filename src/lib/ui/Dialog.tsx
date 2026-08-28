import { createEffect, onCleanup, type JSX } from 'solid-js';
import { ModalController } from './modal';

export type DialogProps = {
	readonly open: boolean;
	readonly onClose: () => void;
	readonly labelledBy: string;
	readonly id?: string;
	readonly testId?: string;
	readonly restoreFocus?: boolean;
	readonly children: JSX.Element;
};

export function Dialog(props: DialogProps): JSX.Element {
	let dialogEl: HTMLDivElement | undefined;
	const modal = new ModalController();

	createEffect(() => {
		modal.sync(
			props.open,
			{ container: dialogEl ?? null },
			{ onEscape: props.onClose, restoreInvoker: props.restoreFocus !== false },
		);
	});
	onCleanup(() => modal.destroy());

	return (
		<div
			id={props.id}
			class="app-modal-backdrop"
			classList={{ open: props.open }}
			data-testid={props.testId}
			aria-hidden={!props.open}
			onClick={(event) => {
				if (event.target === event.currentTarget) {
					props.onClose();
				}
			}}
		>
			<div
				class="app-modal-dialog"
				role="dialog"
				aria-modal="true"
				aria-labelledby={props.labelledBy}
				ref={(element) => {
					dialogEl = element;
				}}
			>
				{props.children}
			</div>
		</div>
	);
}
