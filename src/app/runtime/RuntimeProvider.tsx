import { onCleanup, type JSX } from 'solid-js';
import { AppRuntimeProvider } from './context';
import { createAppRuntime } from './index';

export { AppRuntimeProvider, useAppRuntime } from './context';

export function AppRoot(props: { readonly children: JSX.Element }): JSX.Element {
	const runtime = createAppRuntime();
	onCleanup(() => runtime.dispose());
	return <AppRuntimeProvider runtime={runtime}>{props.children}</AppRuntimeProvider>;
}
