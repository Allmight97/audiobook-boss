import { createSignal, onCleanup, onMount, splitProps, type JSX } from 'solid-js';
import { Button, type ButtonTone } from './Button';

export type SplitButtonProps = {
	readonly variant?: 'default' | 'compact';
	readonly tone?: ButtonTone;
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
	const [local, rest] = splitProps(props, ['class', 'type', 'children']);
	return (
		<button
			{...rest}
			type={local.type ?? 'button'}
			class={`abb-split-option${local.class ? ` ${local.class}` : ''}`}
		>
			{local.children}
		</button>
	);
}

export function SplitButton(props: SplitButtonProps): JSX.Element {
	const [open, setOpen] = createSignal(false);
	let dropdown: HTMLDivElement | undefined;
	let toggle: HTMLButtonElement | undefined;
	const tone = () => props.tone ?? 'primary';
	const variant = () => props.variant ?? 'default';

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

	function close(): void {
		setOpen(false);
	}

	return (
		<div
			class={`abb-split-button${variant() === 'compact' ? ' abb-split-button-compact' : ''}`}
			data-testid={props.testId}
		>
			<Button
				id={props.mainId}
				tone={tone()}
				class="abb-split-main"
				onClick={() => props.onMainClick()}
			>
				{props.mainLabel}
			</Button>
			<Button
				id={props.caretId}
				tone={tone()}
				class="abb-split-caret"
				aria-expanded={open()}
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
