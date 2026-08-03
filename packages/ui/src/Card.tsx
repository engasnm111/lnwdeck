import React from "react";

export interface CardProps {
  title?: string;
  subtitle?: string;
  action?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
}

export function Card({
  title,
  subtitle,
  action,
  children,
  className = "",
}: CardProps) {
  const classes = ["ui-card", className].filter(Boolean).join(" ");
  return (
    <section className={classes}>
      {(title || action) && (
        <header className="ui-card-header">
          {title && (
            <h3 className="ui-card-title">
              {title}
              {subtitle && <span className="ui-card-subtitle">{subtitle}</span>}
            </h3>
          )}
          {action}
        </header>
      )}
      {children}
    </section>
  );
}
