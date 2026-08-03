export interface ToggleProps {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  hint?: string;
  disabled?: boolean;
  id?: string;
}

/**
 * A controlled switch. It has no internal state and no default-checked mode:
 * the rendered position always reflects the value the caller read from the
 * backend.
 */
export function Toggle({
  label,
  checked,
  onChange,
  hint,
  disabled,
  id,
}: ToggleProps) {
  return (
    <label className="ui-toggle" htmlFor={id}>
      <input
        id={id}
        type="checkbox"
        role="switch"
        checked={checked}
        disabled={disabled}
        aria-label={label}
        onChange={(event) => onChange(event.target.checked)}
      />
      <span className="ui-toggle-text">
        <span className="ui-toggle-label">{label}</span>
        {hint && <span className="ui-toggle-hint">{hint}</span>}
      </span>
    </label>
  );
}
