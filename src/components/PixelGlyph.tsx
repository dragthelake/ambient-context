// Title bar glyphs as pixel art, traced pixel for pixel from the reference
// screenshot rather than set in a typeface. A typographic "×" in a modern
// sans is the wrong shape at this size: the period ones are bitmap
// letterforms with 2px strokes and hard diagonals, and no amount of
// font-size gets a smooth glyph to read like one.
//
// Each pattern is one character per source pixel, "#" for ink. The SVG is
// sized so one source pixel is one CSS pixel, which keeps it exact at 1x
// and doubles cleanly on a retina screen.

/// The close X: an 8x8 letterform with 2px diagonals, not a multiplication
/// sign. Symmetrical about both axes, and even on both, which is what lets
/// it centre exactly in the title bar button (see .titlebar-button). The
/// waist is two rows because the diagonals are two pixels wide; a one-row
/// waist pinches it.
export const CLOSE_GLYPH = [
  "##....##",
  ".##..##.",
  "..####..",
  "...##...",
  "...##...",
  "..####..",
  ".##..##.",
  "##....##",
];

export function PixelGlyph({ pattern }: { pattern: string[] }) {
  const width = pattern[0].length;
  const height = pattern.length;
  return (
    <svg
      className="pixel-glyph"
      viewBox={`0 0 ${width} ${height}`}
      width={width}
      height={height}
      shapeRendering="crispEdges"
      aria-hidden="true"
      focusable="false"
    >
      {pattern.map((row, y) =>
        [...row].map((cell, x) =>
          cell === "#" ? (
            <rect key={`${x}-${y}`} x={x} y={y} width="1" height="1" />
          ) : null,
        ),
      )}
    </svg>
  );
}
