import type { AudiobookMetadata } from "../types/metadata";
import { getCurrentCoverArt } from "./coverArt";

export function readMetadataForm(): Partial<AudiobookMetadata> {
  const getElementValue = (id: string): string => {
    const element = document.getElementById(id) as HTMLInputElement;
    return element?.value?.trim() || "";
  };

  const metadata: Partial<AudiobookMetadata> = {};

  const title = getElementValue("meta-title");
  const author = getElementValue("meta-author");
  const narrator = getElementValue("meta-narrator");
  const year = getElementValue("meta-year");
  const genre = getElementValue("meta-genre");
  const series = getElementValue("meta-series");
  const seriesPart = getElementValue("meta-series-part");
  const description = getElementValue("meta-description");

  if (title) {
    metadata.title = title;
    metadata.album = title; // Album derived from title
  }
  if (author) metadata.artist = author; // Map author -> artist for backend
  if (narrator) metadata.composer = narrator; // Map narrator -> composer for backend
  if (year) {
    const yearNum = parseInt(year, 10);
    if (!isNaN(yearNum)) metadata.date = yearNum; // Map year -> date for backend
  }
  if (genre) metadata.genre = genre;
  if (series) {
    // TODO: Persist MVNM (series name) when backend supports it
    // For now, append to album if series is provided
    if (metadata.album) {
      metadata.album = `${metadata.album} (${series}${seriesPart ? " " + seriesPart : ""})`;
    }
    metadata.series = series;
  }
  if (seriesPart) metadata.series_part = seriesPart;
  if (description) metadata.description = description;

  const coverBytes = getCurrentCoverArt();
  if (coverBytes && coverBytes.length > 0) {
    metadata.cover_art = coverBytes;
  }

  return metadata;
}
