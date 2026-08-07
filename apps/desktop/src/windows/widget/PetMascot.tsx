import { useEffect, useState } from "react";
import { fetchPetSpritesheetUrl } from "../../lib/native";
import { petMoodLabel, type PetMood, type PetReaction } from "./petState";
import { useI18n } from "../../lib/i18n";
import "./PetMascot.css";

/** A community pet installed locally and selected for the widget. */
export interface ImportedPet {
  id: string;
  displayName: string;
  spriteVersionNumber: number;
}

/**
 * Animated robot pet for the floating widget's pet layout.
 *
 * Without an imported pet the stage shows the self-authored inline SVG robot.
 * With one, it shows the imported pet's spritesheet loaded over IPC as a Blob
 * object URL (custom URI schemes are blocked by WebView2 on http dev origins),
 * animated by CSS `steps()` frame cycling — no remote asset and no JavaScript
 * animation loop. Either way the artwork is decorative: the svg/atlas is
 * hidden from assistive technology and the mood is exposed as plain text in
 * the status line. All animation is CSS on transform/opacity/background,
 * disabled wholesale under `prefers-reduced-motion`.
 *
 * While the widget is unlocked the stage doubles as a Tauri drag region so the
 * pet can be moved like the header. The stage contains no controls, so nothing
 * interactive is ever swallowed by the drag region.
 */
export function PetMascot({
  mood,
  reaction,
  locked,
  imported,
}: {
  mood: PetMood;
  reaction: PetReaction;
  locked: boolean;
  imported: ImportedPet | null;
}) {
  const { t } = useI18n();
  const [spriteUrl, setSpriteUrl] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setSpriteUrl(null);
    if (!imported) return undefined;
    void fetchPetSpritesheetUrl(imported.id)
      .then((url) => {
        if (!cancelled) setSpriteUrl(url);
      })
      .catch(() => {
        if (!cancelled) setSpriteUrl(null);
      });
    return () => {
      cancelled = true;
    };
  }, [imported?.id]); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <div
      className={`pet-stage pet-mood-${mood}${
        reaction === "celebrate" ? " pet-react-celebrate" : ""
      }`}
      data-locked={locked ? "true" : "false"}
      title={imported ? imported.displayName : undefined}
      {...(locked ? {} : { "data-tauri-drag-region": "" })}
    >
      {imported && spriteUrl ? (
        <div
          className="pet-atlas"
          data-sprite-version={imported.spriteVersionNumber || 1}
          style={{
            backgroundImage: `url(${spriteUrl})`,
          }}
          aria-hidden="true"
        />
      ) : imported && !spriteUrl ? (
        <div className="pet-atlas pet-atlas-missing" aria-hidden="true" />
      ) : (
        <svg
          className="pet-svg"
          viewBox="0 0 120 112"
          aria-hidden="true"
          focusable="false"
        >
          <g className="pet-body">
            <rect className="pet-torso" x="46" y="64" width="28" height="30" rx="9" />
            <rect className="pet-foot" x="42" y="90" width="8" height="8" rx="3.5" />
            <rect className="pet-foot" x="70" y="90" width="8" height="8" rx="3.5" />
          </g>
          <g className="pet-head">
            <rect className="pet-ear pet-ear-l" x="28" y="8" width="14" height="28" rx="7" />
            <rect className="pet-ear pet-ear-r" x="78" y="8" width="14" height="28" rx="7" />
            <rect className="pet-face" x="32" y="18" width="56" height="52" rx="16" />
            <line className="pet-antenna" x1="60" y1="20" x2="60" y2="4" />
            <circle className="pet-antenna-tip" cx="60" cy="4" r="4" />
            <g className="pet-eyes">
              <circle className="pet-eye" cx="47" cy="42" r="6" />
              <circle className="pet-eye" cx="73" cy="42" r="6" />
              <path className="pet-eye-closed pet-eye-closed-l" d="M41 42 h12" />
              <path className="pet-eye-closed pet-eye-closed-r" d="M67 42 h12" />
            </g>
            <g className="pet-mouths">
              <path className="pet-mouth pet-mouth-happy" d="M50 55 Q60 63 70 55" />
              <path
                className="pet-mouth pet-mouth-worried"
                d="M50 57 Q55 52 60 57 Q65 62 70 57"
              />
              <ellipse
                className="pet-mouth pet-mouth-critical"
                cx="60"
                cy="56"
                rx="7"
                ry="6"
              />
              <path className="pet-mouth pet-mouth-stale" d="M50 57 L70 57" />
              <path className="pet-mouth pet-mouth-error" d="M50 59 Q60 52 70 59" />
            </g>
            <g className="pet-alarm">
              <path d="M22 28 L14 24" />
              <path d="M98 28 L106 24" />
              <path d="M26 66 L18 70" />
              <path d="M94 66 L102 70" />
            </g>
          </g>
          <g className="pet-sleep-z">
            <path className="pet-z pet-z-l" d="M86 70 l10 0 l-10 12 l10 0" />
            <path className="pet-z pet-z-s" d="M98 58 l7 0 l-7 9 l7 0" />
          </g>
          <path
            className="pet-sparkle"
            d="M90 15 l2 5 5 2 -5 2 -2 5 -2 -5 -5 -2 5 -2 z"
          />
        </svg>
      )}
      <p className="pet-status" role="status">
        {t("widget.quotaMood", { mood: petMoodLabel(mood, t) })}
      </p>
    </div>
  );
}
