import { createSignal, onSettled, omit } from 'solid-js';
import type { JSX } from '@solidjs/web';
import { Button } from './Button';

export type SplitButtonProps = {
	readonly mainId?: string;
	readonly caretId?: string;
	readonly dropdownId?: string;
	readonly testId?: string;
	readonly onMainClick: () => void;
	readonly mainLabel: JSX.Element;
	readonly children: (helpers: { close: () => void }) => JSX.Element;
};

export type SplitButtonOptionProps = JSX.ButtonHTMLAttributes<HTMLButtonElement>;

function Option(props: SplitButtonOptionProps): JSX.Element {
	const rest = omit(props, 'class', 'type', 'children');
	return (
		<button {...rest} type={props.type ?? 'button'} class={['abb-split-option', props.class]}>
			{props.children}
		</button>
	);
}

export function SplitButton(props: SplitButtonProps): JSX.Element {
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

	function close(): void {
		setOpen(false);
	}

	return (
		<div class="abb-split-button" data-testid={props.testId}>
			<Button
				id={props.mainId}
				tone="primary"
				class="abb-split-main"
				onClick={() => props.onMainClick()}
			>
				{props.mainLabel}
			</Button>
			<Button
				id={props.caretId}
				tone="primary"
				class="abb-split-caret"
				aria-expanded={open() ? 'true' : 'false'}
				ref={toggle}
				onClick={(event) => {
					event.stopPropagation();
					setOpen(!open());
				}}
			>
				▼
			</Button>
			<div
				id={props.dropdownId}
				class={`abb-split-dropdown${open() ? ' open' : ''}`}
				hidden={!open()}
				ref={dropdown}
			>
				{props.children({ close })}
			</div>
		</div>
	);
}

SplitButton.Option = Option;
