import { EmptyTable, SectionHeader } from "./SectionViewPrimitives";

export function AlertsView() {
  return (
    <>
      <SectionHeader
        eyebrow="MONITORING RULES"
        title="Alerts"
        description="Review configured conditions for token and position events."
        stats={["ACTIVE", "TRIGGERED", "MUTED"]}
      />
      <div className="section-view__body">
        <EmptyTable
          title="Alert rules"
          detail="No rules configured"
          columns={["Alert", "Condition", "Scope", "State", "Last triggered"]}
          emptyTitle="No alerts configured"
          emptyDetail="There are no alert rules in this workspace."
        />
      </div>
    </>
  );
}
