import { EmptyTable, SectionHeader } from "./SectionViewPrimitives";

export function ReplaysView() {
  return (
    <>
      <SectionHeader
        eyebrow="RECORDED SESSIONS"
        title="Replays"
        description="Review deterministic and strategy decisions from recorded stream events."
        stats={["SESSIONS", "EVENTS", "LAST RUN"]}
      />
      <div className="section-view__body">
        <EmptyTable
          title="Replay history"
          detail="Recorded event sessions"
          columns={["Session", "Window", "Events", "Strategy", "Status", "Created"]}
          emptyTitle="No replay data"
          emptyDetail="A replay requires recorded discovery events."
        />
      </div>
    </>
  );
}
