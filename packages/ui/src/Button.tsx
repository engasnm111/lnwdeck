import React from "react";

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "secondary";
  children: React.ReactNode;
}

export function Button({ variant = "primary", children, className = "", ...props }: ButtonProps) {
  const variantClass = variant === "secondary" ? "ui-button-secondary" : "ui-button-primary";
  return (
    <button className={`ui-button ${variantClass} ${className}`} {...props}>
      {children}
    </button>
  );
}
