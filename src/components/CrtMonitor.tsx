import type { ReactNode } from "react";

// A CRT drawn the way the reference figure draws one: not line art, but a
// shaded object. One light, high and to the left, which decides every
// value in here. Top and left edges take the highlight, bottom and right
// take the shading, and both the case and the base cast a shadow down and
// to the right.
//
// The case, neck and base touch. A monitor whose stand floats below it
// reads as three shapes rather than one object, which was the main thing
// wrong with the earlier drawing.
//
// The screen is a foreignObject so whatever it frames stays ordinary HTML,
// which is how the ASCII eye keeps animating without knowing it is inside
// an SVG.
export function CrtMonitor({
  children,
  glowing,
}: {
  children: ReactNode;
  glowing: boolean;
}) {
  return (
    <svg
      className="crt"
      viewBox="0 0 300 250"
      role="img"
      aria-label="A computer monitor"
      preserveAspectRatio="xMidYMid meet"
    >
      {/* Cast shadows go down first, so everything else sits on them. */}
      <rect className="crt-shadow" x="98" y="239" width="126" height="6" rx="2" />
      <rect className="crt-shadow" x="42" y="200" width="234" height="8" rx="2" />

      {/* Base slab, then the neck, then the case over the top of both, so
          the joins are covered rather than drawn. */}
      <g className="crt-body">
        <rect x="88" y="224" width="124" height="13" rx="2" />
        <path d="M132 194 h36 l14 30 h-64 z" />
      </g>
      <path className="crt-lit" d="M88 224 h124 v3 h-124 z" />
      <path className="crt-shade" d="M88 234 h124 v3 h-124 z" />

      {/* The case. A 3px corner, not the soft 8px it had: a moulded plastic
          bezel of this era is very nearly square. The tube behind it is
          4:3, so the case is nearly square too rather than widescreen. */}
      <rect className="crt-body" x="30" y="8" width="240" height="192" rx="3" />

      {/* The light: two bands inside the top and left edges. */}
      <path className="crt-lit" d="M33 11 h234 v4 h-230 v182 h-4 z" />
      {/* And the shading it implies, inside the bottom and right. */}
      <path className="crt-shade" d="M267 11 v186 h-234 v-4 h230 v-182 z" />

      {/* The screen sits in a recess, so its shadow falls on the top and
          left inside edges: the opposite of the case, and what makes it
          read as cut into the bezel rather than stuck on. */}
      <rect className="crt-recess" x="48" y="24" width="204" height="153" />
      <rect className="crt-lit" x="50" y="175" width="204" height="2" />
      <rect className="crt-lit" x="252" y="24" width="2" height="153" />
      <rect
        className="crt-screen-fill"
        x="50"
        y="26"
        width="200"
        height="149"
        data-on={glowing ? "true" : "false"}
      />
      <foreignObject x="52" y="28" width="196" height="145">
        <div className={glowing ? "crt-screen is-on" : "crt-screen"}>
          {children}
        </div>
      </foreignObject>

      {/* Chin: vents on the left, a moulded strip and the power light on
          the right, each with its own one-pixel light and shade. */}
      <g className="crt-vent">
        <path d="M60 185 h46" />
        <path d="M60 190 h46" />
      </g>
      <rect className="crt-recess" x="194" y="183" width="24" height="9" rx="1" />
      <circle
        className={glowing ? "crt-led is-on" : "crt-led"}
        cx="234"
        cy="187"
        r="4.5"
      />
    </svg>
  );
}
