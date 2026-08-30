export type NativeDropTarget = 'cover' | 'files' | 'ignore';

const COVER_ART_IMAGE_EXTENSION_HINT_PATTERN = /\.(jpg|jpeg|png|webp)$/i;

export function nativeDropLooksLikeCoverArt(paths: ReadonlyArray<string>): boolean {
	return paths.some((path) => COVER_ART_IMAGE_EXTENSION_HINT_PATTERN.test(path));
}

export function nativeDropTargetAtPoint(
	position: { readonly x: number; readonly y: number },
	coverArtArea: HTMLElement | null,
	fileManagementContainer: HTMLElement | null,
): NativeDropTarget {
	if (coverArtArea && pointInElement(position, coverArtArea)) {
		return 'cover';
	}
	if (fileManagementContainer && pointInElement(position, fileManagementContainer)) {
		return 'files';
	}
	return 'ignore';
}

function pointInElement(
	position: { readonly x: number; readonly y: number },
	element: HTMLElement,
): boolean {
	const rect = element.getBoundingClientRect();
	return (
		position.x >= rect.left &&
		position.x <= rect.right &&
		position.y >= rect.top &&
		position.y <= rect.bottom
	);
}
