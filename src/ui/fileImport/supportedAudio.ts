export const SUPPORTED_AUDIO_EXTENSIONS = ['mp3', 'm4a', 'm4b', 'aac', 'wav', 'flac'] as const;

export const SUPPORTED_AUDIO_FORMATS_TEXT = SUPPORTED_AUDIO_EXTENSIONS.join(', ');

export const SUPPORTED_AUDIO_SUPPORT_TEXT = `Supports: ${SUPPORTED_AUDIO_FORMATS_TEXT}`;

export const SUPPORTED_AUDIO_DROP_SUFFIXES = SUPPORTED_AUDIO_EXTENSIONS.map(
	(extension) => `.${extension}`,
);

export function isSupportedAudioPath(path: string): boolean {
	const lowerPath = path.toLowerCase();
	return SUPPORTED_AUDIO_DROP_SUFFIXES.some((suffix) => lowerPath.endsWith(suffix));
}
