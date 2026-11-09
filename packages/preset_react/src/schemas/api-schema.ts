import { z } from 'zod';
import { yearBindingSchema } from './config-schema.ts';

// -- Preset browsing --

export const presetSummarySchema = z.object({
  id: z.string(),
  dataVersion: z.number(),
  displayName: z.string(),
  feedUrlHints: z
    .array(z.string().min(1))
    .min(1)
    .refine((hints) => new Set(hints).size === hints.length, {
      message: 'feedUrlHints must not contain repeats',
    }),
  playlistCount: z.number(),
});

export const presetMetaSchema = z.object({
  dataVersion: z.number(),
  id: z.string(),
  podcastGuid: z.string().optional(),
  feedUrls: z.array(z.string()),
  yearGroupedEpisodes: z.boolean().default(false),
  playlists: z.array(z.string()),
});

export const presetIdentifiersSchema = z.object({
  id: z.string(),
  podcastGuid: z.string().nullish(),
  feedUrls: z.array(z.string()),
});

export type PresetIdentifiers = z.infer<typeof presetIdentifiersSchema>;

// -- Feed episodes --

export const feedEpisodeSchema = z.object({
  id: z.number(),
  title: z.string(),
  description: z.string().nullish(),
  guid: z.string().nullish(),
  publishedAt: z.string().nullish(),
  seasonNumber: z.number().nullish(),
  episodeNumber: z.number().nullish(),
  imageUrl: z.string().nullish(),
});

// -- Preview --

export const previewEpisodeSchema = z.object({
  id: z.number(),
  title: z.string(),
  publishedAt: z.string().nullish(),
  seasonNumber: z.number().nullish(),
  episodeNumber: z.number().nullish(),
  extractedDisplayName: z.string().nullish(),
});

export type PreviewGroup = {
  id: string;
  displayName: string;
  sortKey: string | number;
  episodeCount: number;
  episodes: z.infer<typeof previewEpisodeSchema>[];
  subGroups?: PreviewGroup[];
};

export const previewGroupSchema: z.ZodType<PreviewGroup> = z.lazy(() =>
  z.object({
    id: z.string(),
    displayName: z.string(),
    sortKey: z.union([z.string(), z.number()]),
    episodeCount: z.number(),
    episodes: z.array(previewEpisodeSchema),
    subGroups: z.array(previewGroupSchema).optional(),
  }),
);

export const claimedEpisodeSchema = z.object({
  id: z.number(),
  title: z.string(),
  seasonNumber: z.number().nullish(),
  episodeNumber: z.number().nullish(),
  claimedBy: z.string(),
});

export const playlistDebugSchema = z.object({
  filterMatched: z.number(),
  episodeCount: z.number(),
  claimedByOthersCount: z.number(),
});

export const previewPlaylistSchema = z.object({
  id: z.string(),
  displayName: z.string(),
  sortKey: z.union([z.string(), z.number()]),
  resolverType: z.string().nullish(),
  episodeCount: z.number(),
  yearBinding: yearBindingSchema.default('none'),
  groups: z.array(previewGroupSchema).optional(),
  claimedByOthers: z.array(claimedEpisodeSchema).optional().default([]),
  debug: playlistDebugSchema.optional(),
});

export const previewDebugSchema = z.object({
  totalEpisodes: z.number(),
  groupedEpisodes: z.number(),
  ungroupedEpisodes: z.number(),
  excludedEpisodes: z.number().optional().default(0),
});

export const previewResultSchema = z.object({
  playlists: z.array(previewPlaylistSchema),
  ungrouped: z.array(previewEpisodeSchema),
  excluded: z.array(previewEpisodeSchema).optional().default([]),
  resolverType: z.string().nullish(),
  debug: previewDebugSchema.optional(),
});

// -- Podcast search --

export const podcastSearchResultSchema = z.object({
  trackName: z.string(),
  artistName: z.string(),
  artworkUrl100: z.string().optional(),
  feedUrl: z.string().optional(),
  trackCount: z.number().optional(),
  primaryGenreName: z.string().optional(),
});

export const podcastSearchResponseSchema = z.object({
  resultCount: z.number(),
  results: z.array(podcastSearchResultSchema),
});

// -- Inferred types --

export type PresetSummary = z.infer<typeof presetSummarySchema>;
export type PresetMeta = z.infer<typeof presetMetaSchema>;
export type FeedEpisode = z.infer<typeof feedEpisodeSchema>;
export type PreviewEpisode = z.infer<typeof previewEpisodeSchema>;
// PreviewGroup is defined above (recursive type)
export type PreviewPlaylist = z.infer<typeof previewPlaylistSchema>;
export type ClaimedEpisode = z.infer<typeof claimedEpisodeSchema>;
export type PlaylistDebug = z.infer<typeof playlistDebugSchema>;
export type PreviewDebug = z.infer<typeof previewDebugSchema>;
export type PreviewResult = z.infer<typeof previewResultSchema>;
export type PodcastSearchResult = z.infer<typeof podcastSearchResultSchema>;
export type PodcastSearchResponse = z.infer<typeof podcastSearchResponseSchema>;
