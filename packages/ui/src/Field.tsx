import React from "react";

export interface FieldProps {
  label: string;
  htmlFor?: string;
  hint?: string;
  error?: string;
  children: React.ReactNode;
}

export function Field({ label, htmlFor, hint, error, children }: FieldProps) {
  return (
    <div className="ui-field">
      <label className="ui-field-label" htmlFor={htmlFor}>
        {label}
      </label>
      {children}
      {hint && !error && <span className="ui-field-hint">{hint}</span>}
      {error && (
        <span className="ui-field-error" role="alert">
          {error}
        </span>
      )}
    </div>
  );
}
