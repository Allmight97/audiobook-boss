import { splitProps, type JSX } from 'solid-js';

export type ButtonTone = 'primary' | 'secondary';

export type ButtonProps = {
	readonly tone?: ButtonTone;
	readonly busy?: boolean;
} & JSX.ButtonHTMLAttributes<HTMLButtonElement>;

export function Button(props: ButtonProps): JSX.Element {
	const [local, rest] = splitProps(props, [
		'tone',
		'busy',
		'class',
		'disabled',
		'type',
		'children',
	]);
	const tone = () => local.tone ?? 'secondary';
	return (
		<button
			{...rest}
			type={local.type ?? 'button'}
			disabled={local.disabled || local.busy}
			aria-busy={local.busy || undefined}
			class={`abb-button ${tone() === 'primary' ? 'abb-button-primary' : 'abb-button-secondary'}${local.class ? ` ${local.class}` : ''}`}
		>
			{local.children}
		</button>
	);
}
