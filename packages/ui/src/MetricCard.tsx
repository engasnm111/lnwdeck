import React from "react";

export interface MetricCardProps {
  title: string;
  value: string | number;
  subtitle?: string;
  badge?: React.ReactNode;
  icon?: React.ReactNode;
}

export function MetricCard({ title, value, subtitle, badge, icon }: MetricCardProps) {
  return (
    <div className="ui-metric-card" role="region" aria-label={title}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <span className="ui-metric-card-title">{title}</span>
        {icon && <span style={{ color: "var(--text-muted)" }}>{icon}</span>}
      </div>
      <div style={{ display: "flex", alignItems: "baseline", gap: "0.5rem" }}>
        <span className="ui-metric-card-value">{value}</span>
        {badge}
      </div>
      {subtitle && <span className="ui-metric-card-subtitle">{subtitle}</span>}
    </div>
  );
}
