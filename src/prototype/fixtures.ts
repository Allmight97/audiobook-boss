export type BookStatus = 'encoding' | 'ready' | 'no-cover' | 'queued';

export type EditorTabId = 'metadata' | 'facts' | 'chapters' | 'output';

export const EDITOR_TABS: ReadonlyArray<{ id: EditorTabId; label: string }> = [
	{ id: 'metadata', label: 'Metadata' },
	{ id: 'facts', label: 'Facts' },
	{ id: 'chapters', label: 'Chapters' },
	{ id: 'output', label: 'Output' },
];

export type BookFixture = {
	id: number;
	title: string;
	author: string;
	duration: string;
	size: string;
	codec: string;
	status: BookStatus;
	chapters: number;
	gradient: string;
	subtitle: string;
	narrator: string;
	series: string;
	seriesNumber: string;
	genre: string;
	bitrate: string;
	sampleRate: string;
	channels: string;
	chapterTitles: readonly string[];
	outputFormat: string;
	outputPath: string;
	outputNaming: string;
};

export const BOOK_FIXTURES: BookFixture[] = [
	{
		id: 0,
		title: 'The Way of Kings',
		author: 'Brandon Sanderson',
		duration: '18:22:41',
		size: '512 MB',
		codec: 'AAC-LC',
		status: 'encoding',
		chapters: 75,
		gradient: 'linear-gradient(160deg,#4338ca,#1e3a8a)',
		subtitle: 'Brandon Sanderson · 18:22:41 · 75 chapters',
		narrator: 'Michael Kramer, Kate Reading',
		series: 'The Stormlight Archive',
		seriesNumber: '1',
		genre: 'Fantasy',
		bitrate: '64 kbps',
		sampleRate: '44.1 kHz',
		channels: 'Stereo',
		chapterTitles: ['Prologue', 'The First Step', 'Into the Unknown'],
		outputFormat: 'M4B (FDK HE-AAC)',
		outputPath: '/Audiobooks/Stormlight/The Way of Kings.m4b',
		outputNaming: 'ABS naming',
	},
	{
		id: 1,
		title: 'A Change of Plans',
		author: 'Dannie Ella',
		duration: '9:04:12',
		size: '261 MB',
		codec: 'AAC-LC',
		status: 'ready',
		chapters: 41,
		gradient: 'linear-gradient(160deg,#9d174d,#4c0519)',
		subtitle: 'Dannie Ella · 9:04:12 · 41 chapters',
		narrator: 'Dannie Ella',
		series: '',
		seriesNumber: '',
		genre: 'Memoir',
		bitrate: '64 kbps',
		sampleRate: '44.1 kHz',
		channels: 'Mono',
		chapterTitles: ['Opening', 'The Call', 'Departure'],
		outputFormat: 'M4B (FDK HE-AAC)',
		outputPath: '/Audiobooks/Memoir/A Change of Plans.m4b',
		outputNaming: 'ABS naming',
	},
	{
		id: 2,
		title: 'The Future',
		author: 'Naomi Alderman',
		duration: '11:41:03',
		size: '322 MB',
		codec: 'AAC-LC',
		status: 'no-cover',
		chapters: 52,
		gradient: 'linear-gradient(160deg,#065f46,#022c22)',
		subtitle: 'Naomi Alderman · 11:41:03 · 52 chapters',
		narrator: 'Naomi Alderman',
		series: '',
		seriesNumber: '',
		genre: 'Science Fiction',
		bitrate: '64 kbps',
		sampleRate: '44.1 kHz',
		channels: 'Stereo',
		chapterTitles: ['Part One', 'Signal', 'Aftermath'],
		outputFormat: 'M4B (FDK HE-AAC)',
		outputPath: '/Audiobooks/Sci-Fi/The Future.m4b',
		outputNaming: 'ABS naming',
	},
	{
		id: 3,
		title: 'The Martian',
		author: 'Andy Weir',
		duration: '10:53:29',
		size: '299 MB',
		codec: 'MP3',
		status: 'ready',
		chapters: 38,
		gradient: 'linear-gradient(160deg,#92400e,#451a03)',
		subtitle: 'Andy Weir · 10:53:29 · 38 chapters',
		narrator: 'R. C. Bray',
		series: '',
		seriesNumber: '',
		genre: 'Science Fiction',
		bitrate: '128 kbps',
		sampleRate: '44.1 kHz',
		channels: 'Stereo',
		chapterTitles: ['Sol 6', 'Sol 17', 'Sol 61'],
		outputFormat: 'M4B (FDK HE-AAC)',
		outputPath: '/Audiobooks/Sci-Fi/The Martian.m4b',
		outputNaming: 'ABS naming',
	},
	{
		id: 4,
		title: 'Emergent Strategy',
		author: 'adrienne maree brown',
		duration: '12:45:02',
		size: '351 MB',
		codec: 'AAC-LC',
		status: 'queued',
		chapters: 12,
		gradient: 'linear-gradient(160deg,#155e75,#082f49)',
		subtitle: 'adrienne maree brown · 12:45:02 · 12 chapters',
		narrator: 'adrienne maree brown',
		series: '',
		seriesNumber: '',
		genre: 'Nonfiction',
		bitrate: '64 kbps',
		sampleRate: '44.1 kHz',
		channels: 'Mono',
		chapterTitles: ['Introduction', 'Fractals', 'Interdependence'],
		outputFormat: 'M4B (FDK HE-AAC)',
		outputPath: '/Audiobooks/Nonfiction/Emergent Strategy.m4b',
		outputNaming: 'ABS naming',
	},
];

export const STATUS_BADGE: Record<
	BookStatus,
	{ label: string; class: 'ok' | 'info' | 'warn' | 'mut' }
> = {
	encoding: { label: 'encoding', class: 'info' },
	ready: { label: 'ready', class: 'ok' },
	'no-cover': { label: 'no cover', class: 'warn' },
	queued: { label: 'queued', class: 'mut' },
};

export const MULTI_SELECTION_IDS = [1, 2, 3] as const;

/** Desktop prototype scope — horizontal scroll below this width, not a mobile layout. */
export const MIN_SHELL_WIDTH_REM = 56.25;
