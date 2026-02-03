import { describe, expect, it, beforeEach, vi } from 'vitest';

let coverArtBytes: number[] | null = null;
let coverRemoval = false;
let hasCustomCoverArt = false;

vi.mock('../coverArt', () => ({
  getCurrentCoverArt: () => coverArtBytes,
  getHasCustomCoverArt: () => hasCustomCoverArt,
  isCoverArtRemovalRequested: () => coverRemoval,
  setCoverArt: () => {},
}));

import { readMetadataForm } from '../metadataForm';

const setField = (id: string, value: string) => {
  const el = document.getElementById(id) as HTMLInputElement | HTMLTextAreaElement | null;
  if (el) el.value = value;
};

describe('readMetadataForm (single mode)', () => {
  beforeEach(() => {
    document.body.innerHTML = `
      <div id="metadata-form">
        <input id="meta-title" />
        <select id="meta-title-action"><option value="keep">Keep</option><option value="blank">Blank</option></select>
        <input id="meta-author" />
        <select id="meta-author-action"><option value="keep">Keep</option><option value="blank">Blank</option></select>
        <input id="meta-narrator" />
        <select id="meta-narrator-action"><option value="keep">Keep</option><option value="blank">Blank</option></select>
        <input id="meta-year" />
        <select id="meta-year-action"><option value="keep">Keep</option><option value="blank">Blank</option></select>
        <input id="meta-genre" />
        <select id="meta-genre-action"><option value="keep">Keep</option><option value="blank">Blank</option></select>
        <input id="meta-series" />
        <select id="meta-series-action"><option value="keep">Keep</option><option value="blank">Blank</option></select>
        <input id="meta-series-part" />
        <select id="meta-series-part-action"><option value="keep">Keep</option><option value="blank">Blank</option></select>
        <input id="meta-subseries" />
        <select id="meta-subseries-action"><option value="keep">Keep</option><option value="blank">Blank</option></select>
        <input id="meta-subseries-part" />
        <select id="meta-subseries-part-action"><option value="keep">Keep</option><option value="blank">Blank</option></select>
        <textarea id="meta-description"></textarea>
        <select id="meta-description-action"><option value="keep">Keep</option><option value="blank">Blank</option></select>
      </div>
    `;
    coverArtBytes = null;
    coverRemoval = false;
    hasCustomCoverArt = false;
  });

  it('maps form fields to metadata and includes cover art bytes', () => {
    setField('meta-title', 'Title');
    setField('meta-author', 'Author');
    setField('meta-narrator', 'Narrator');
    setField('meta-year', '2024');
    setField('meta-genre', 'Fiction');
    setField('meta-series', 'Series');
    setField('meta-series-part', '2');
    setField('meta-subseries', 'Sub-series');
    setField('meta-subseries-part', '4');
    setField('meta-description', 'Desc');
    coverArtBytes = [1, 2, 3];

    const metadata = readMetadataForm({ mode: 'single' });

    expect(metadata).toMatchObject({
      title: 'Title',
      album: 'Title',
      artist: 'Author',
      composer: 'Narrator',
      date: 2024,
      genre: 'Fiction',
      series: 'Series',
      series_part: '2',
      subseries: 'Sub-series',
      subseries_part: '4',
      description: 'Desc',
      cover_art: [1, 2, 3],
    });
  });

  it('emits empty cover_art array when removal requested', () => {
    coverRemoval = true;

    const metadata = readMetadataForm({ mode: 'single' });

    expect(metadata.cover_art).toEqual([]);
  });

  it('includes empty strings for clearable metadata fields', () => {
    const metadata = readMetadataForm({ mode: 'single' });

    expect(metadata.series).toBe('');
    expect(metadata.series_part).toBe('');
    expect(metadata.subseries).toBe('');
    expect(metadata.subseries_part).toBe('');
    expect(metadata.description).toBe('');
  });
});

describe('readMetadataForm (multi mode)', () => {
  beforeEach(() => {
    document.body.innerHTML = `
      <div id="metadata-form" data-multi-select="true">
        <input id="meta-title" />
        <select id="meta-title-action"><option value="keep">Keep</option><option value="blank">Blank</option></select>
        <input id="meta-author" />
        <select id="meta-author-action"><option value="keep">Keep</option><option value="blank">Blank</option></select>
        <input id="meta-narrator" />
        <select id="meta-narrator-action"><option value="keep">Keep</option><option value="blank">Blank</option></select>
        <input id="meta-year" />
        <select id="meta-year-action"><option value="keep">Keep</option><option value="blank">Blank</option></select>
        <input id="meta-genre" />
        <select id="meta-genre-action"><option value="keep">Keep</option><option value="blank">Blank</option></select>
        <input id="meta-series" />
        <select id="meta-series-action"><option value="keep">Keep</option><option value="blank">Blank</option></select>
        <input id="meta-series-part" />
        <select id="meta-series-part-action"><option value="keep">Keep</option><option value="blank">Blank</option></select>
        <input id="meta-subseries" />
        <select id="meta-subseries-action"><option value="keep">Keep</option><option value="blank">Blank</option></select>
        <input id="meta-subseries-part" />
        <select id="meta-subseries-part-action"><option value="keep">Keep</option><option value="blank">Blank</option></select>
        <textarea id="meta-description"></textarea>
        <select id="meta-description-action"><option value="keep">Keep</option><option value="blank">Blank</option></select>
      </div>
    `;
    coverArtBytes = null;
    coverRemoval = false;
    hasCustomCoverArt = false;
  });

  it('uses bulk blank actions for multi-select', () => {
    const yearAction = document.getElementById('meta-year-action') as HTMLSelectElement;
    yearAction.value = 'blank';

    const metadata = readMetadataForm({ mode: 'multi', onlyDirty: true });

    expect(metadata.date).toBe(0);
  });

  it('applies edited values in multi-select mode', () => {
    const titleInput = document.getElementById('meta-title') as HTMLInputElement;
    titleInput.value = 'New Title';
    titleInput.dataset.dirty = 'true';

    const metadata = readMetadataForm({ mode: 'multi', onlyDirty: true });

    expect(metadata).toMatchObject({
      title: 'New Title',
      album: 'New Title',
    });
  });

  it('ignores cover art changes in multi-select', () => {
    coverArtBytes = [1, 2, 3];
    coverRemoval = true;
    hasCustomCoverArt = true;

    const metadata = readMetadataForm({ mode: 'multi', onlyDirty: true });

    expect(metadata.cover_art).toBeUndefined();
  });
});
