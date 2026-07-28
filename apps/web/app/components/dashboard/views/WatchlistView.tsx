import { EmptyTable, SectionHeader } from "./SectionViewPrimitives";

export function WatchlistView() {
  return (
    <>
      <SectionHeader
        eyebrow="SAVED TOKENS"
        title="Watchlist"
        description="Keep selected discovery candidates together for review."
        stats={["WATCHED", "APPROVED", "STRATEGY MATCHES"]}
      />
      <div className="section-view__body">
        <EmptyTable
          title="Watched tokens"
          detail="Saved in this workspace"
          columns={["Token", "First pass", "Risk", "Rating", "Strategy", "Added"]}
          emptyTitle="Watchlist is empty"
          emptyDetail="Add a token from the discovery stream when candidates arrive."
        />
      </div>
    </>
  );
}
