import { writable } from "svelte/store";

export const TAG_FIELDS = [
  "title",
  "album",
  "artist",
  "albumArtist",
  "composer",
  "series",
  "part",
  "subseries",
  "subpart",
  "tsoa",
  "year",
  "genre",
] as const;

export type TagField = (typeof TAG_FIELDS)[number];
export type TagPreviewValues = Record<TagField, string>;

const EMPTY_VALUES: TagPreviewValues = {
  title: "",
  album: "",
  artist: "",
  albumArtist: "",
  composer: "",
  series: "",
  part: "",
  subseries: "",
  subpart: "",
  tsoa: "",
  year: "",
  genre: "",
};

export const tagPreviewValuesStore = writable<TagPreviewValues>({
  ...EMPTY_VALUES,
});

export function createEmptyTagPreviewValues(): TagPreviewValues {
  return { ...EMPTY_VALUES };
}

export function setTagPreviewValues(values: TagPreviewValues): void {
  tagPreviewValuesStore.set(values);
}
