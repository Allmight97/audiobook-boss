/**
 * Output path building and sanitization utilities
 */
import type { AudiobookMetadata } from "../../types/metadata";
import { getJobType } from "../jobControls";
import { getOutputNamingConfig, getState } from "./state";

/**
 * Sanitizes a string for use in filenames by replacing problematic characters
 */
export function sanitizeFilename(
  input: string,
  options: { preserveCommas?: boolean } = {}
): string {
  const preserveCommas = options.preserveCommas ?? false;
  let value = input.replace(/:/g, " - ");
  if (!preserveCommas) {
    value = value.replace(/,/g, " - ");
  }
  return value
    .replace(/[/\\*?"<>|]/g, " ")
    .split(/\s+/)
    .filter(Boolean)
    .join(" ");
}

function buildAbsTitle(
  title: string,
  seriesPart: string | undefined,
  year: number | string | undefined,
  includeYear: boolean
): string {
  if (includeYear && year !== undefined) {
    if (seriesPart) {
      return `Book ${seriesPart} - ${String(year)} - ${title}`;
    }
    return `${String(year)} - ${title}`;
  }

  if (seriesPart) {
    return `Book ${seriesPart} - ${title}`;
  }
  return title;
}

function buildSimpleFilename(
  title: string,
  year: number | string | undefined,
  includeYear: boolean
): string {
  let base = title;
  if (includeYear && year !== undefined) {
    base = `${base} (${year})`;
  }
  return `${base}.m4b`;
}

/**
 * Calculates the full output path based on current settings for PREVIEW
 */
export function calculateOutputPath(metadata: AudiobookMetadata): string {
  const state = getState();
  const naming = getOutputNamingConfig();
  let basePath = state.outputDirectory || "[Output Directory]";
  const jobType = getJobType();

  if (jobType === "batch") {
    basePath += "/(Batch Output Folder)";
    const placeholderAuthor = "[Author]";
    const placeholderSeries = "[Series]";
    const placeholderTitle = "[Title]";
    const placeholderSeriesPart = "#";
    const placeholderYear = "YYYY";

    if (naming.absCompatible) {
      const absTitle = buildAbsTitle(
        placeholderTitle,
        placeholderSeriesPart,
        naming.includeYear ? placeholderYear : undefined,
        naming.includeYear
      );
      return `${basePath}/${placeholderAuthor}/${placeholderSeries}/${absTitle}/${absTitle}.m4b`;
    }

    const filename = buildSimpleFilename(
      placeholderTitle,
      naming.includeYear ? placeholderYear : undefined,
      naming.includeYear
    );
    return `${basePath}/${filename}`;
  }

  const author =
    sanitizeFilename(metadata.artist || "Unknown Author", {
      preserveCommas: true,
    }) || "Unknown Author";
  const title = sanitizeFilename(metadata.title || "Untitled") || "Untitled";
  const series = sanitizeFilename(metadata.series || "");
  const year = typeof metadata.date === "number" ? metadata.date : undefined;

  if (naming.absCompatible) {
    let subdirPath = `${basePath}/${author}`;
    if (series) {
      subdirPath += `/${series}`;
    }
    const rawSeriesPart = metadata.series_part || "";
    const seriesPartValue = sanitizeFilename(rawSeriesPart.split("/")[0]).trim();
    const seriesPart = seriesPartValue.length > 0 ? seriesPartValue : undefined;
    const absTitle = buildAbsTitle(title, seriesPart, year, naming.includeYear);
    return `${subdirPath}/${absTitle}/${absTitle}.m4b`;
  }

  const filename = buildSimpleFilename(
    title,
    naming.includeYear ? year : undefined,
    naming.includeYear
  );
  return `${basePath}/${filename}`;
}
