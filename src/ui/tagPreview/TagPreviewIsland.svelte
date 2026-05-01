<script lang="ts">
  import { onMount } from "svelte";
  import { initTagPreview } from "../tagPreview";
  import { tagPreviewValues, type TagField } from "./state.svelte";

  type TagRow = {
    field: TagField;
    label: string;
    title: string;
  };

  const leftRows: TagRow[] = [
    {
      field: "title",
      label: "Title (Book Title)",
      title:
        "The audiobook's title. Displayed as track/episode name in most players.",
    },
    {
      field: "album",
      label: "Album (Book Title)",
      title:
        "Groups all chapters under one album. Plex and Audiobookshelf use this for the book name.",
    },
    {
      field: "artist",
      label: "Artist (Author)",
      title: "The book's author. Shows as primary artist in most players.",
    },
    {
      field: "albumArtist",
      label: "Album Artist (Author)",
      title:
        "Used for library grouping. Keeps all books by an author together.",
    },
    {
      field: "composer",
      label: "Composer (Narrator)",
      title:
        "Stores the narrator. Plex shows this in audiobook details; Audiobookshelf displays it as narrator.",
    },
  ];

  const rightRows: TagRow[] = [
    {
      field: "series",
      label: "SERIES (Series)",
      title:
        "Series name tag (series). Written to freeform SERIES (----:com.apple.iTunes:SERIES) for ABS/Plex-compatible scanners.",
    },
    {
      field: "part",
      label: "SERIES-PART (Book #)",
      title:
        "Series number tag (series-part). Written to freeform SERIES-PART (----:com.apple.iTunes:SERIES-PART) for ABS/Plex-compatible scanners.",
    },
    {
      field: "subseries",
      label: "SERIES (Sub-series)",
      title:
        "Secondary series name. Stored as the second entry in the SERIES list for ABS/Plex.",
    },
    {
      field: "subpart",
      label: "SERIES-PART (Sub-series #)",
      title:
        "Secondary series number. Stored as the second entry in SERIES-PART for ABS/Plex.",
    },
    {
      field: "tsoa",
      label: "TSOA (Title Sort Order)",
      title:
        "Auto-generated sort key. Forces Plex to sort by series, then book number, then title.",
    },
    {
      field: "year",
      label: "©day (Publication Date)",
      title: "Publication date stored as YYYY or YYYY-MM when available.",
    },
    {
      field: "genre",
      label: "TCON (Genre)",
      title: "Genre tag. Used for library filtering and display.",
    },
  ];

  onMount(() => {
    initTagPreview();
  });
</script>

<div class="tag-grid mb-2">
  <div class="tag-column">
    {#each leftRows as row}
      <div class="tag-row" title={row.title}>
        <span class="tag-name">{row.label}</span>
        <span class="tag-value" data-field={row.field}>
          {tagPreviewValues[row.field] || "—"}
        </span>
      </div>
    {/each}
  </div>

  <div class="tag-column">
    {#each rightRows as row}
      <div class="tag-row" title={row.title}>
        <span class="tag-name">{row.label}</span>
        <span class="tag-value" data-field={row.field}>
          {tagPreviewValues[row.field] || "—"}
        </span>
      </div>
    {/each}
  </div>
</div>

<style>
	.tag-grid {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0.75rem;
		padding: 0.5rem;
		border: 1px solid var(--border-primary);
		border-radius: 0.375rem;
		background: var(--bg-input);
	}

	.tag-column {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
	}

	.tag-row {
		display: flex;
		align-items: baseline;
		gap: 0.5rem;
		font-size: 0.7rem;
		line-height: 1.4;
	}

	.tag-name {
		min-width: 70px;
		color: var(--text-muted);
		font-family: var(--font-mono);
		white-space: nowrap;
	}

	.tag-value {
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		color: var(--text-primary);
	}
</style>
