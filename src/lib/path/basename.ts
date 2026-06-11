export type PathBasenameFallback = 'path' | 'empty';

type PathBasenameOptions = {
	fallback?: PathBasenameFallback;
	trimTrailingSeparators?: boolean;
};

export function pathBasename(path: string, options: PathBasenameOptions = {}): string {
	const { fallback = 'path', trimTrailingSeparators = false } = options;
	const segments = path
		.split(/[\\/]/)
		.filter((segment) => (trimTrailingSeparators ? segment.length > 0 : segment !== ''));
	const last = segments[segments.length - 1];
	if (last) {
		return last;
	}
	return fallback === 'empty' ? '' : path;
}

export function pathSegments(path: string): string[] {
	return path.split(/[\\/]/).filter(Boolean);
}
