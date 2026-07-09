export type BookStatus = 'encoding' | 'ready' | 'no-cover' | 'queued';

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
