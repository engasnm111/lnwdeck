import React from "react";

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "secondary" | "danger" | "ghost";
  size?: "regular" | "small";
  children: React.ReactNode;
}

export function Button({
  variant = "secondary",
  size = "regular",
  children,
  className = "",
  type = "button",
  ...props
}: ButtonProps) {
  const classes = [
    "ui-button",
    variant === "primary" ? "ui-button-primary" : "",
    variant === "danger" ? "ui-button-danger" : "",
    variant === "ghost" ? "ui-button-ghost" : "",
    size === "small" ? "ui-button-small" : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <button className={classes} type={type} {...props}>
      {children}
    </button>
  );
}
