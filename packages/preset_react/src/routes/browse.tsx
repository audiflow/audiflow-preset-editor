import { useState } from 'react';
import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import { usePresets } from '@/api/queries.ts';
import { Button } from '@/components/ui/button.tsx';
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
} from '@/components/ui/card.tsx';
import { Badge } from '@/components/ui/badge.tsx';
import { SearchDialog, type PodcastSelection } from '@/components/podcast-search/search-dialog.tsx';
import { Plus, Loader2, Search } from 'lucide-react';

export const Route = createFileRoute('/browse')({
  component: BrowseScreen,
});

function BrowseScreen() {
  const navigate = useNavigate();
  const { data: presets, isLoading, error } = usePresets();
  const [searchOpen, setSearchOpen] = useState(false);

  const handleSearchSelect = (selection: PodcastSelection) => {
    void navigate({
      to: '/editor',
      search: {
        feedUrl: selection.feedUrl,
        displayName: selection.trackName,
      },
    });
  };

  return (
    <div className="container mx-auto max-w-4xl p-6">
      <BrowseHeader
        navigate={navigate}
        onSearchOpen={() => setSearchOpen(true)}
      />

      <SearchDialog
        open={searchOpen}
        onOpenChange={setSearchOpen}
        onSelect={handleSearchSelect}
      />

      {isLoading && <LoadingState />}
      {error && <ErrorState message={error.message} />}
      {presets && presets.length === 0 && <EmptyState />}
      {presets && 0 < presets.length && (
        <PresetList presets={presets} navigate={navigate} />
      )}
    </div>
  );
}

function BrowseHeader({
  navigate,
  onSearchOpen,
}: {
  navigate: ReturnType<typeof useNavigate>;
  onSearchOpen: () => void;
}) {
  const { t } = useTranslation('common');

  return (
    <div className="flex items-center justify-between mb-6">
      <h1 className="text-2xl font-bold">{t('appTitle')}</h1>
      <div className="flex gap-2">
        <Button variant="outline" onClick={onSearchOpen}>
          <Search className="mr-2 h-4 w-4" />
          {t('searchPodcasts')}
        </Button>
        <Button onClick={() => void navigate({ to: '/editor' })}>
          <Plus className="mr-2 h-4 w-4" />
          {t('createNew')}
        </Button>
      </div>
    </div>
  );
}

function LoadingState() {
  return (
    <div className="flex justify-center py-12">
      <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
    </div>
  );
}

function ErrorState({ message }: { message: string }) {
  const { t } = useTranslation('common');

  return (
    <div className="text-center py-12 text-destructive">
      {t('loadPresetsFailed', { error: message })}
    </div>
  );
}

function EmptyState() {
  const { t } = useTranslation('common');

  return (
    <div className="text-center py-12 text-muted-foreground">
      {t('noPresetsFound')}
    </div>
  );
}

function PresetList({
  presets,
  navigate,
}: {
  presets: Array<{
    id: string;
    displayName: string;
    feedUrlHints: string[];
    playlistCount: number;
  }>;
  navigate: ReturnType<typeof useNavigate>;
}) {
  return (
    <div className="grid gap-4">
      {presets.map((preset) => (
        <PresetCard
          key={preset.id}
          preset={preset}
          navigate={navigate}
        />
      ))}
    </div>
  );
}

function PresetCard({
  preset,
  navigate,
}: {
  preset: {
    id: string;
    displayName: string;
    feedUrlHints: string[];
    playlistCount: number;
  };
  navigate: ReturnType<typeof useNavigate>;
}) {
  const { t } = useTranslation('feed');

  return (
    <Card
      className="cursor-pointer hover:bg-accent/50 transition-colors"
      onClick={() =>
        void navigate({ to: '/editor/$id', params: { id: preset.id } })
      }
    >
      <CardHeader>
        <div className="flex items-center justify-between">
          <CardTitle className="text-lg">{preset.displayName}</CardTitle>
          <Badge variant="secondary">
            {t('playlists', { count: preset.playlistCount })}
          </Badge>
        </div>
        {preset.feedUrlHints.map((hint) => (
          <CardDescription key={hint}>{hint}</CardDescription>
        ))}
      </CardHeader>
    </Card>
  );
}
