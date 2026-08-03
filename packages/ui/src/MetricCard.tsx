import React from "react";

export interface MetricCardProps {
  title: string;
  value: string | number;
  subtitle?: string;
  badge?: React.ReactNode;
  icon?: React.ReactNode;
}

export function MetricCard({
  title,
  value,
  subtitle,
  badge,
  icon,
}: MetricCardProps) {
  return (
    <div className="ui-metric-card" role="group" aria-label={title}>
      <div className="ui-metric-card-head">
        <span className="ui-metric-card-title">{title}</span>
        {icon}
      </div>
      <div className="ui-metric-card-row">
        <span className="ui-metric-card-value">{value}</span>
        {badge}
      </div>
      {subtitle && <span className="ui-metric-card-subtitle">{subtitle}</span>}
    </div>
  );
}
