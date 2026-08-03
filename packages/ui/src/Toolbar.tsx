import React from "react";

export interface ToolbarProps {
  children: React.ReactNode;
  label: string;
}

export function Toolbar({ children, label }: ToolbarProps) {
  return (
    <div className="ui-toolbar" role="toolbar" aria-label={label}>
      {children}
    </div>
  );
}

export function ToolbarSpacer() {
  return <span className="ui-toolbar-spacer" />;
}
