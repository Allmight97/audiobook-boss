export type PathBasenameFallback = 'path' | 'empty';

type PathBasenameOptions = {
	fallback?: PathBasenameFallback;
};

export function pathBasename(path: string, options: PathBasenameOptions = {}): string {
	const { fallback = 'path' } = options;
	const segments = path.split(/[\\/]/).filter((segment) => segment !== '');
	const last = segments[segments.length - 1];
	if (last) {
		return last;
	}
	return fallback === 'empty' ? '' : path;
}

export function pathSegments(path: string): string[] {
	return path.split(/[\\/]/).filter(Boolean);
}
