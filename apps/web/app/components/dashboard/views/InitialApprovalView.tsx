import { EmptyTable, SectionHeader } from "./SectionViewPrimitives";

export function InitialApprovalView() {
  return (
    <>
      <SectionHeader
        eyebrow="DETERMINISTIC SCREENING"
        title="Initial approval"
        description="Review completed first-pass decisions and their recorded reason codes."
        stats={["APPROVED", "REJECTED", "PENDING"]}
      />
      <div className="section-view__body">
        <EmptyTable
          title="First-pass results"
          detail="Completed assessments"
          columns={["Token", "Decision", "Risk", "Rating", "Checks", "Assessed"]}
          emptyTitle="No first-pass results"
          emptyDetail="Results will appear after candidates complete deterministic screening."
        />
      </div>
    </>
  );
}
