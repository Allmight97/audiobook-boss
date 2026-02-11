/**
 * Tag Preview module
 *
 * Calculates TSOA (sort key) and updates the tag preview grid
 * based on metadata input values.
 */
import { mount } from "svelte";
import TagPreviewIsland from "./tagPreview/TagPreviewIsland.svelte";
import {
  TAG_FIELDS,
  createEmptyTagPreviewValues,
  setTagPreviewValues,
  type TagField,
  type TagPreviewValues,
} from "./tagPreview/state";

/**
 * Pads a part number to 2 digits for proper sorting
 */
function padPart(num: string): string {
  const n = parseInt(num, 10);
  if (isNaN(n) || n < 1) return "00";
  return n < 10 ? `0${n}` : `${n}`;
}

/**
 * Calculates the TSOA (Album Sort) tag value
 * Format: "Series PP - Title" where PP is zero-padded part number
 * Returns empty string if series or title is missing
 */
export function calculateTSOA(series: string, part: string, title: string): string {
  const trimmedSeries = series.trim();
  const trimmedTitle = title.trim();

  if (!trimmedSeries || !trimmedTitle) return "";

  const paddedPart = padPart(part);
  return `${trimmedSeries} ${paddedPart} - ${trimmedTitle}`;
}

/**
 * Tag field mappings from metadata inputs to tag preview data-field attributes
 */
const TAG_FIELD_MAPPINGS: Record<TagField, () => string> = {
  title: () => getInputValue("meta-title"),
  album: () => getInputValue("meta-title"), // Album derived from title
  artist: () => getInputValue("meta-author"),
  albumArtist: () => getInputValue("meta-author"), // Same as artist
  composer: () => getInputValue("meta-narrator"),
  series: () => getInputValue("meta-series"),
  part: () => getInputValue("meta-series-part"),
  subseries: () => getInputValue("meta-subseries"),
  subpart: () => getInputValue("meta-subseries-part"),
  year: () => getInputValue("meta-year"),
  genre: () => getInputValue("meta-genre"),
  tsoa: () =>
    calculateTSOA(
      getInputValue("meta-series"),
      getInputValue("meta-series-part"),
      getInputValue("meta-title")
    ),
};

let mountedPreviewRoot: HTMLElement | null = null;

/**
 * Gets the trimmed value from an input element
 */
function getInputValue(id: string): string {
  const element = document.getElementById(id) as HTMLInputElement | HTMLTextAreaElement | null;
  return element?.value?.trim() || "";
}

function mountTagPreviewIsland(): void {
  const previewRoot = document.getElementById("tag-preview-root");
  if (!previewRoot) return;

  if (
    mountedPreviewRoot === previewRoot &&
    previewRoot.childElementCount > 0
  ) {
    return;
  }

  mount(TagPreviewIsland, { target: previewRoot });
  mountedPreviewRoot = previewRoot;
}

function getTagPreviewValues(): TagPreviewValues {
  const values = createEmptyTagPreviewValues();
  for (const field of TAG_FIELDS) {
    values[field] = TAG_FIELD_MAPPINGS[field]();
  }
  return values;
}

/**
 * Updates all tag preview fields
 */
export function updateTagPreview(): void {
  setTagPreviewValues(getTagPreviewValues());
}

/**
 * Sets up event listeners on metadata inputs to update tag preview in real-time
 */
export function initTagPreview(): void {
  mountTagPreviewIsland();

  // Input IDs that affect tag preview
  const inputIds = [
    "meta-title",
    "meta-author",
    "meta-narrator",
    "meta-series",
    "meta-series-part",
    "meta-subseries",
    "meta-subseries-part",
    "meta-year",
    "meta-genre",
  ];

  for (const id of inputIds) {
    const element = document.getElementById(id);
    if (element) {
      // Use 'input' event for real-time updates as user types
      element.addEventListener("input", updateTagPreview);
    }
  }

  // Initial update
  updateTagPreview();
}
