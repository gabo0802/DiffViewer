import React, { useId } from "react";

type IconProps = React.SVGProps<SVGSVGElement> & {
  size?: number;
};

export function LoadingIcon({ size = 18, ...props }: IconProps) {
  return (
    <svg
      viewBox="0 0 24 24"
      width={size}
      height={size}
      aria-hidden="true"
      {...props}
    >
      {Array.from({ length: 8 }, (_, index) => (
        <circle
          key={index}
          cx="12"
          cy="2"
          r="0"
          fill="currentColor"
          transform={index === 0 ? undefined : `rotate(${index * 45} 12 12)`}
        >
          <animate
            attributeName="r"
            begin={`${index * 0.125}s`}
            calcMode="spline"
            dur="1s"
            keySplines="0.2 0.2 0.4 0.8;0.2 0.2 0.4 0.8;0.2 0.2 0.4 0.8"
            repeatCount="indefinite"
            values="0;2;0;0"
          />
        </circle>
      ))}
    </svg>
  );
}

export function SaveIcon({ size = 18, ...props }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" width={size} height={size} aria-hidden="true" {...props}>
      <path
        fill="currentColor"
        d="M21 7v14H3V3h14zm-2 .85L16.15 5H5v14h14zm-4.875 9.275Q15 16.25 15 15t-.875-2.125T12 12t-2.125.875T9 15t.875 2.125T12 18t2.125-.875M6 10h9V6H6zM5 7.85V19V5z"
      />
    </svg>
  );
}

export function EditIcon({ size = 18, ...props }: IconProps) {
  return (
    <svg viewBox="0 0 24 24" width={size} height={size} aria-hidden="true" {...props}>
      <g fill="currentColor">
        <path d="M8 7a1 1 0 0 1-1 1H6a1 1 0 0 0-1 1v9a1 1 0 0 0 1 1h9a1 1 0 0 0 1-1v-1a1 1 0 0 1 2 0v1a3 3 0 0 1-3 3H6a3 3 0 0 1-3-3V9a3 3 0 0 1 3-3h1a1 1 0 0 1 1 1" />
        <path d="m14.596 5.011l4.392 4.392l-6.28 6.303A1 1 0 0 1 12 16H9a1 1 0 0 1-1-1v-3a1 1 0 0 1 .294-.708zm6.496-2.103a3.097 3.097 0 0 1 .165 4.203l-.164.18l-.693.694l-4.387-4.387l.695-.69a3.1 3.1 0 0 1 4.384 0" />
      </g>
    </svg>
  );
}

export function PrevIcon({ size = 18, ...props }: IconProps) {
  return (
    <svg viewBox="0 0 32 32" width={size} height={size} aria-hidden="true" {...props}>
      <path fill="currentColor" d="m16 8l1.43 1.393L11.85 15H24v2H11.85l5.58 5.573L16 24l-8-8z" />
      <path fill="currentColor" d="M16 30a14 14 0 1 1 14-14a14.016 14.016 0 0 1-14 14m0-26a12 12 0 1 0 12 12A12.014 12.014 0 0 0 16 4" />
    </svg>
  );
}

export function NextIcon({ size = 18, ...props }: IconProps) {
  return (
    <svg viewBox="0 0 32 32" width={size} height={size} aria-hidden="true" {...props}>
      <path fill="currentColor" d="m16 8l-1.43 1.393L20.15 15H8v2h12.15l-5.58 5.573L16 24l8-8z" />
      <path fill="currentColor" d="M16 30a14 14 0 1 1 14-14a14.016 14.016 0 0 1-14 14m0-26a12 12 0 1 0 12 12A12.014 12.014 0 0 0 16 4" />
    </svg>
  );
}

export function ThemeIcon({
  mode,
  size = 18,
  ...props
}: IconProps & {
  mode: "dark" | "light";
}) {
  const maskA = useId().replace(/:/g, "");
  const maskB = useId().replace(/:/g, "");
  const isLight = mode === "light";

  return (
    <svg viewBox="0 0 24 24" width={size} height={size} aria-hidden="true" {...props}>
      <defs>
        <mask id={maskA}>
          <circle cx={isLight ? 11 : 7.5} cy="7.5" r={isLight ? 6.5 : 5.5} fill="#fff" />
          <circle cx={isLight ? 7.5 : 11} cy="7.5" r={isLight ? 5.5 : 6.5} />
        </mask>
        <mask id={maskB}>
          <g fill="#fff">
            <circle cx="12" cy={isLight ? 15 : 9} r="5.5" transform="rotate(-45 12 12)" />
            {isLight &&
              [-120, -70, -20, 30].map((rotation) => (
                <path
                  key={rotation}
                  d="M12.62 20.62h3l-1.5 2.5Z"
                  transform={`rotate(${rotation} 14.12 14.12)`}
                />
              ))}
          </g>
          <path d="M-4.97 12l18.38 -18.38l10.61 10.61l-18.38 18.38Z" />
        </mask>
      </defs>
      <g fill="currentColor">
        <path d="M0 0h24v24H0z" mask={`url(#${maskA})`} />
        <path d="M0 0h24v24H0z" mask={`url(#${maskB})`} />
      </g>
      {isLight && (
        <path
          fill="none"
          stroke="currentColor"
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth="2"
          d="M23 12h-22"
          transform="rotate(-45 12 12)"
        />
      )}
    </svg>
  );
}
