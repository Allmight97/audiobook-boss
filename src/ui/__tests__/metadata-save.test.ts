import { describe, expect, it, beforeEach, vi } from 'vitest';

let coverArtBytes: number[] | null = null;
let coverRemoval = false;

vi.mock('../../ui/coverArt', () => ({
  getCurrentCoverArt: () => coverArtBytes,
  isCoverArtRemovalRequested: () => coverRemoval,
}));

import { collectMetadataFromForm } from '../../main';

const setField = (id: string, value: string) => {
  const el = document.getElementById(id) as HTMLInputElement | HTMLTextAreaElement | null;
  if (el) el.value = value;
};

describe('collectMetadataFromForm', () => {
  beforeEach(() => {
    document.body.innerHTML = `
      <input id="meta-title" />
      <input id="meta-author" />
      <input id="meta-narrator" />
      <input id="meta-year" />
      <input id="meta-genre" />
      <input id="meta-series" />
      <input id="meta-series-part" />
      <textarea id="meta-description"></textarea>
    `;
    coverArtBytes = null;
    coverRemoval = false;
  });

  it('maps form fields to metadata and includes cover art bytes', () => {
    setField('meta-title', 'Title');
    setField('meta-author', 'Author');
    setField('meta-narrator', 'Narrator');
    setField('meta-year', '2024');
    setField('meta-genre', 'Fiction');
    setField('meta-series', 'Series');
    setField('meta-series-part', '2');
    setField('meta-description', 'Desc');
    coverArtBytes = [1, 2, 3];

    const metadata = collectMetadataFromForm();

    expect(metadata).toMatchObject({
      title: 'Title',
      album: 'Title',
      artist: 'Author',
      composer: 'Narrator',
      date: 2024,
      genre: 'Fiction',
      series: 'Series',
      series_part: '2',
      description: 'Desc',
      cover_art: [1, 2, 3],
    });
  });

  it('emits empty cover_art array when removal requested', () => {
    coverRemoval = true;

    const metadata = collectMetadataFromForm();

    expect(metadata.cover_art).toEqual([]);
  });

  it('includes empty strings for clearable metadata fields', () => {
    const metadata = collectMetadataFromForm();

    expect(metadata.series).toBe('');
    expect(metadata.series_part).toBe('');
    expect(metadata.description).toBe('');
  });
});
