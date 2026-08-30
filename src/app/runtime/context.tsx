import { createContext, useContext, type JSX } from 'solid-js';
import type { AppRuntime } from './types';

const AppRuntimeContext = createContext<AppRuntime>();

export function AppRuntimeProvider(props: {
	readonly runtime: AppRuntime;
	readonly children: JSX.Element;
}): JSX.Element {
	return (
		<AppRuntimeContext.Provider value={props.runtime}>{props.children}</AppRuntimeContext.Provider>
	);
}

export function useAppRuntime(): AppRuntime {
	const runtime = useContext(AppRuntimeContext);
	if (!runtime) {
		throw new Error('App runtime is not mounted');
	}
	return runtime;
}
