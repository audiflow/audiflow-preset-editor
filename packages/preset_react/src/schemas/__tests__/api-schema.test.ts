import { describe, expect, it } from 'vitest';
import {
  presetSummarySchema,
  presetMetaSchema,
  feedEpisodeSchema,
  previewEpisodeSchema,
  previewGroupSchema,
  previewPlaylistSchema,
  previewDebugSchema,
  previewResultSchema,
} from '../api-schema';

describe('presetSummarySchema', () => {
  it('parses valid summary', () => {
    const result = presetSummarySchema.parse({
      id: 'podcast-abc',
      dataVersion: 1,
      displayName: 'My Podcast',
      feedUrlHints: ['https://example.com/feed.xml', 'https://old.example.com/feed.xml'],
      playlistCount: 3,
    });
    expect(result.id).toBe('podcast-abc');
    expect(result.feedUrlHints).toEqual([
      'https://example.com/feed.xml',
      'https://old.example.com/feed.xml',
    ]);
    expect(result.dataVersion).toBe(1);
    expect(result.playlistCount).toBe(3);
  });

  it.each([
    ['empty list', []],
    ['blank hint', ['']],
    ['repeated hint', ['https://a.example.com', 'https://a.example.com']],
  ])('rejects feedUrlHints with %s', (_label, feedUrlHints) => {
    expect(() =>
      presetSummarySchema.parse({
        id: 'x',
        dataVersion: 1,
        displayName: 'X',
        feedUrlHints,
        playlistCount: 1,
      }),
    ).toThrow();
  });

  it('rejects the legacy scalar feedUrlHint', () => {
    expect(() =>
      presetSummarySchema.parse({
        id: 'x',
        dataVersion: 1,
        displayName: 'X',
        feedUrlHint: 'https://example.com/feed.xml',
        playlistCount: 1,
      }),
    ).toThrow();
  });

  it('rejects missing fields', () => {
    expect(() =>
      presetSummarySchema.parse({ id: 'x' }),
    ).toThrow();
  });
});

describe('presetMetaSchema', () => {
  it('parses minimal meta with defaults', () => {
    const result = presetMetaSchema.parse({
      dataVersion: 1,
      id: 'podcast-abc',
      feedUrls: ['https://example.com/feed.xml'],
      playlists: ['main'],
    });
    expect(result.yearGroupedEpisodes).toBe(false);
    expect(result.podcastGuid).toBeUndefined();
  });

  it('parses full meta', () => {
    const result = presetMetaSchema.parse({
      dataVersion: 2,
      id: 'podcast-xyz',
      podcastGuid: 'guid-123',
      feedUrls: ['https://a.com', 'https://b.com'],
      yearGroupedEpisodes: true,
      playlists: ['main', 'bonus'],
    });
    expect(result.podcastGuid).toBe('guid-123');
    expect(result.yearGroupedEpisodes).toBe(true);
    expect(result.playlists).toEqual(['main', 'bonus']);
  });
});

describe('feedEpisodeSchema', () => {
  it('parses minimal episode', () => {
    const result = feedEpisodeSchema.parse({
      id: 0,
      title: 'Episode 1',
    });
    expect(result.id).toBe(0);
    expect(result.title).toBe('Episode 1');
    expect(result.description).toBeUndefined();
    expect(result.seasonNumber).toBeUndefined();
  });

  it('parses full episode', () => {
    const result = feedEpisodeSchema.parse({
      id: 42,
      title: 'Episode 42',
      description: 'A great episode',
      guid: 'abc-123',
      publishedAt: '2024-01-15T00:00:00Z',
      seasonNumber: 3,
      episodeNumber: 42,
      imageUrl: 'https://example.com/img.jpg',
    });
    expect(result.seasonNumber).toBe(3);
    expect(result.publishedAt).toBe('2024-01-15T00:00:00Z');
  });
});

describe('previewEpisodeSchema', () => {
  it('parses preview episode', () => {
    const result = previewEpisodeSchema.parse({
      id: 1,
      title: 'Episode 1',
    });
    expect(result.id).toBe(1);
    expect(result.seasonNumber).toBeUndefined();
    expect(result.episodeNumber).toBeUndefined();
  });

  it('parses enriched episode with all fields', () => {
    const input = {
      id: 1,
      title: 'S01E03 - Rome',
      publishedAt: '2024-01-15T00:00:00Z',
      seasonNumber: 1,
      episodeNumber: 3,
      extractedDisplayName: 'Rome',
    };
    const result = previewEpisodeSchema.parse(input);
    expect(result.publishedAt).toBe('2024-01-15T00:00:00Z');
    expect(result.seasonNumber).toBe(1);
    expect(result.episodeNumber).toBe(3);
    expect(result.extractedDisplayName).toBe('Rome');
  });

  it('parses minimal episode without optional fields', () => {
    const input = { id: 1, title: 'Episode 1' };
    const result = previewEpisodeSchema.parse(input);
    expect(result.publishedAt).toBeUndefined();
    expect(result.extractedDisplayName).toBeUndefined();
  });
});

describe('previewGroupSchema', () => {
  it('parses preview group', () => {
    const result = previewGroupSchema.parse({
      id: 'season-1',
      displayName: 'Season 1',
      sortKey: 1,
      episodeCount: 10,
      episodes: [
        { id: 1, title: 'Ep 1' },
        { id: 2, title: 'Ep 2' },
      ],
    });
    expect(result.episodeCount).toBe(10);
    expect(result.episodes).toHaveLength(2);
  });

  it('accepts string sortKey', () => {
    const result = previewGroupSchema.parse({
      id: 'alpha',
      displayName: 'Alpha',
      sortKey: 'alpha-key',
      episodeCount: 5,
      episodes: [],
    });
    expect(result.sortKey).toBe('alpha-key');
  });
});

describe('previewPlaylistSchema', () => {
  it('parses playlist without groups', () => {
    const result = previewPlaylistSchema.parse({
      id: 'main',
      displayName: 'Main',
      sortKey: 0,
      episodeCount: 50,
    });
    expect(result.resolverType).toBeUndefined();
    expect(result.groups).toBeUndefined();
  });

  it('parses playlist with groups', () => {
    const result = previewPlaylistSchema.parse({
      id: 'seasons',
      displayName: 'Seasons',
      sortKey: 1,
      resolverType: 'seasonNumber',
      episodeCount: 100,
      groups: [
        {
          id: 's1',
          displayName: 'Season 1',
          sortKey: 1,
          episodeCount: 10,
          episodes: [],
        },
      ],
    });
    expect(result.groups).toHaveLength(1);
  });
});

describe('previewDebugSchema', () => {
  it('parses debug stats', () => {
    const result = previewDebugSchema.parse({
      totalEpisodes: 100,
      groupedEpisodes: 90,
      ungroupedEpisodes: 10,
    });
    expect(result.totalEpisodes).toBe(100);
  });
});

describe('previewResultSchema', () => {
  it('parses empty result', () => {
    const result = previewResultSchema.parse({
      playlists: [],
      ungrouped: [],
    });
    expect(result.resolverType).toBeUndefined();
    expect(result.debug).toBeUndefined();
  });

  it('parses full result', () => {
    const result = previewResultSchema.parse({
      playlists: [
        {
          id: 'main',
          displayName: 'Main',
          sortKey: 0,
          episodeCount: 10,
        },
      ],
      ungrouped: [{ id: 99, title: 'Unmatched' }],
      resolverType: 'seasonNumber',
      debug: {
        totalEpisodes: 11,
        groupedEpisodes: 10,
        ungroupedEpisodes: 1,
      },
    });
    expect(result.playlists).toHaveLength(1);
    expect(result.ungrouped).toHaveLength(1);
    expect(result.resolverType).toBe('seasonNumber');
  });
});

