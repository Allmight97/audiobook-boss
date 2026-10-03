import type { TagPreview } from '../../types/session';

export type TagField =
	| 'title'
	| 'album'
	| 'artist'
	| 'albumArtist'
	| 'composer'
	| 'series'
	| 'part'
	| 'subseries'
	| 'subpart'
	| 'tsoa'
	| 'year'
	| 'genre';

export type TagPreviewValues = Record<TagField, string>;

/** The engine's tag preview under the tag panel's row names. */
export function tagPreviewValues(tags: TagPreview): TagPreviewValues {
	return {
		title: tags.title,
		album: tags.album,
		artist: tags.artist,
		albumArtist: tags.albumArtist,
		composer: tags.composer,
		series: tags.series,
		part: tags.seriesPart,
		subseries: tags.subseries,
		subpart: tags.subseriesPart,
		tsoa: tags.albumSort,
		year: tags.year,
		genre: tags.genre,
	};
}
