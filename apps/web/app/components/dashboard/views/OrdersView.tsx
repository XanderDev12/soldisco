import { EmptyTable, SectionHeader } from "./SectionViewPrimitives";

export function OrdersView() {
  return (
    <>
      <SectionHeader
        eyebrow="ORDER ACTIVITY"
        title="Orders"
        description="Review order intent and submission outcomes when execution is connected."
        stats={["OPEN", "COMPLETED", "FAILED"]}
      />
      <div className="section-view__body">
        <EmptyTable
          title="Order records"
          detail="Execution service not connected"
          columns={["Token", "Side", "Mode", "Amount", "Status", "Submitted"]}
          emptyTitle="Order data unavailable"
          emptyDetail="No execution service is connected."
        />
      </div>
    </>
  );
}
