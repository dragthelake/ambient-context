import type { ReactNode } from "react";

// A CRT drawn the way the reference figure draws one: offset fills, not
// outlines. Three concentric rounded rects, each stepped 3px toward the
// top-left light, leave a white band on the lit edges and a dark band on
// the shaded ones, which is the same construction as every bevel in the
// chrome. Nothing in the drawing carries a stroke, and nothing casts a
// shadow: the reference monitor sits on the dialog face by its own
// bottom-right shading alone.
//
// The tube is near-black even when idle. What changes with power is the
// ink on it, plus the little switch on the pedestal.
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
      viewBox="0 0 300 234"
      role="img"
      aria-label="A computer monitor"
      preserveAspectRatio="xMidYMid meet"
    >
      {/* The case bevel, four fills deep. Each rect is anchored at the
          same top-left corner and shrunk a little more than the one below
          it, so the lower layers surface only along the bottom and right:
          a near-black hairline at the very edge, then a mid-grey band,
          which is the reference's soft ramp from face to outline. The
          white rect shrinks less than the face does, so it surfaces along
          the top and left instead. */}
      <rect className="crt-dark" x="30" y="8" width="240" height="184" rx="6" />
      <rect className="crt-shade" x="30" y="8" width="238.8" height="182.8" rx="5.5" />
      <rect className="crt-lit" x="30" y="8" width="236.5" height="180.5" rx="5.5" />
      <rect className="crt-body" x="33" y="11" width="233.5" height="177.5" rx="4.5" />

      {/* The screen recess is sunken, so its bevel runs opposite to the
          case's: shade falls on the tube's top and left, and a white
          sliver surfaces along its bottom and right, which is what the
          reference shows at the screen's corner. */}
      <rect className="crt-lit" x="44" y="26" width="212" height="150" rx="6" />
      <rect className="crt-shade" x="44" y="26" width="210.5" height="148.5" rx="5.5" />
      <rect
        className="crt-screen-fill"
        x="46"
        y="28"
        width="208"
        height="146"
        rx="5"
        data-on={glowing ? "true" : "false"}
      />
      <foreignObject x="50" y="32" width="200" height="138">
        <div className={glowing ? "crt-screen is-on" : "crt-screen"}>
          {children}
        </div>
      </foreignObject>

      {/* The stand steps down in three pieces: a pedestal slab with
          rounded bottom corners, a narrower neck, and a thin pill foot.
          Each is the dark shape with the body fill inset a hairline
          inside it, so the outline follows every contour instead of
          being drawn on; and each piece butts the one above it, so the
          joins are covered rather than drawn.

          The neck comes first: its top hides behind the slab, and only
          its hairlined sides show between slab and foot. */}
      <rect className="crt-dark" x="94" y="200" width="112" height="17" />
      <rect className="crt-body" x="95.2" y="200" width="109.6" height="17" />

      <path
        className="crt-dark"
        d="M85 192 v8.5 q0 5 5 5 h120 q5 0 5 -5 v-8.5 z"
      />
      <path
        className="crt-body"
        d="M86.2 192 v7.3 q0 4 3.8 4 h120 q3.8 0 3.8 -4 v-7.3 z"
      />

      {/* The switch on the pedestal's right: a raised pill, and the small
          power light beside it, which is the one part of the object that
          answers the recording state. */}
      <rect className="crt-button" x="176" y="194.5" width="24" height="6" rx="3" />
      <rect
        className={glowing ? "crt-led is-on" : "crt-led"}
        x="168.5"
        y="196"
        width="3.5"
        height="3.5"
      />

      <rect className="crt-dark" x="75" y="216" width="150" height="9.5" rx="4.75" />
      <rect className="crt-lit" x="76" y="217" width="148" height="7.5" rx="3.75" />
      <rect className="crt-body" x="76" y="218.5" width="148" height="6" rx="3" />
    </svg>
  );
}
