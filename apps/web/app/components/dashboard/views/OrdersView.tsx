import { executionModePresentation } from "../executionModePresentation";
import type { ExecutionMode } from "../types";
import { EmptyTable, SectionHeader } from "./SectionViewPrimitives";

type OrdersViewProps = {
  mode: ExecutionMode;
};

export function OrdersView({ mode }: OrdersViewProps) {
  const content = executionModePresentation[mode].orders;

  return (
    <>
      <SectionHeader
        eyebrow={content.eyebrow}
        title={content.title}
        description={content.description}
        stats={content.stats}
      />
      <div className="section-view__body">
        <EmptyTable
          title={content.recordTitle}
          detail={content.recordDetail}
          columns={content.columns}
          emptyTitle={content.emptyTitle}
          emptyDetail={content.emptyDetail}
        />
      </div>
    </>
  );
}
