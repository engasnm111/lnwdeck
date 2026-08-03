import React from "react";

export interface BadgeProps {
  children: React.ReactNode;
  tone?: "success" | "warning" | "danger" | "info" | "default";
}

export function Badge({ children, tone = "default" }: BadgeProps) {
  const toneClass = tone !== "default" ? `ui-badge-${tone}` : "ui-badge-info";
  return <span className={`ui-badge ${toneClass}`}>{children}</span>;
}
