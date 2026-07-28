type SectionHeaderProps = {
  eyebrow: string;
  title: string;
  description: string;
  stats: readonly string[];
};

type EmptyTableProps = {
  title: string;
  detail: string;
  columns: readonly string[];
  emptyTitle: string;
  emptyDetail: string;
};

export function SectionHeader({
  eyebrow,
  title,
  description,
  stats,
}: SectionHeaderProps) {
  return (
    <header className="section-view__header">
      <div>
        <span className="eyebrow">{eyebrow}</span>
        <h1 id="view-title" tabIndex={-1}>
          {title}
        </h1>
        <p>{description}</p>
      </div>
      <dl>
        {stats.map((stat) => (
          <div key={stat}>
            <dt>{stat}</dt>
            <dd aria-label={`${stat.toLowerCase()} unavailable`}>—</dd>
          </div>
        ))}
      </dl>
    </header>
  );
}

export function EmptyTable({
  title,
  detail,
  columns,
  emptyTitle,
  emptyDetail,
}: EmptyTableProps) {
  return (
    <div className="section-card">
      <div className="section-card__head">
        <div>
          <h2>{title}</h2>
          <p>{detail}</p>
        </div>
        <span className="status-chip">Empty</span>
      </div>
      <div className="table-scroll">
        <table className="section-table">
          <thead>
            <tr>
              {columns.map((column) => (
                <th key={column} scope="col">
                  {column}
                </th>
              ))}
            </tr>
          </thead>
          <tbody />
        </table>
      </div>
      <div className="section-empty">
        <span aria-hidden="true">◇</span>
        <h2>{emptyTitle}</h2>
        <p>{emptyDetail}</p>
      </div>
    </div>
  );
}
