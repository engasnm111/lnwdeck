import React from "react";

export type BadgeTone = "success" | "warning" | "danger" | "info" | "neutral";

export interface BadgeProps {
  children: React.ReactNode;
  tone?: BadgeTone;
  title?: string;
}

/**
 * A small status pill. The tone is derived from real state by the caller; the
 * default is neutral, so a badge can never accidentally read as healthy.
 */
export function Badge({ children, tone = "neutral", title }: BadgeProps) {
  return (
    <span className={`ui-badge ui-badge-${tone}`} title={title}>
      {children}
    </span>
  );
}
