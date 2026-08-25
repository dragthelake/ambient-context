import { useEffect, useState } from "react";

// Hand-drawn frames, all eleven rows tall so the panel never reflows.
// Awake: the eye idles open, glances, and blinks on a deliberately
// irregular loop. Asleep: a closed lid with lashes, and Zs drifting up.
const OPEN = String.raw`
          .:::::::::::::::::.
      .:::''               '':::.
   .::''       .:#####:.       ''::.
 .::'        :##:::::::##:        '::.
::'         ##::  .-.  ::##         '::
::          ##:: ( @ ) ::##          ::
::.         ##::  '-'  ::##         .::
 '::.        :##:::::::##:        .::'
   '::..       ':#####:'       ..::'
      ':::..               ..:::'
          ':::::::::::::::::'
`;

const GLANCE = String.raw`
          .:::::::::::::::::.
      .:::''               '':::.
   .::''       .:#####:.       ''::.
 .::'        :##:::::::##:        '::.
::'         ##:  .-.    :##         '::
::          ##: ( @ )   :##          ::
::.         ##:  '-'    :##         .::
 '::.        :##:::::::##:        .::'
   '::..       ':#####:'       ..::'
      ':::..               ..:::'
          ':::::::::::::::::'
`;

const BLINK = String.raw`




   ..::::::::::::::::::::::::::::::..
:::::::::::::::::::::::::::::::::::::::
   '':::::::::::::::::::::::::::::''




`;

function sleepFrame(zs: string[]): string {
  const [z0, z1, z2] = zs;
  return `
${z0}
${z1}
${z2}

   '::..                       ..::'
      '':::::::::::::::::::::''
         '      '     '      '
          '     '     '     '



`;
}

const SLEEP: string[] = [
  sleepFrame(["", "", "                          z"]),
  sleepFrame(["", "                            Z", "                          z"]),
  sleepFrame([
    "                               z",
    "                            Z",
    "                          z",
  ]),
  sleepFrame([
    "                               z",
    "                            Z",
    "",
  ]),
  sleepFrame(["                               z", "", ""]),
  sleepFrame(["", "", ""]),
];

// (frame, milliseconds to hold it)
const AWAKE_SEQUENCE: Array<[string, number]> = [
  [OPEN, 2600],
  [BLINK, 110],
  [OPEN, 1900],
  [GLANCE, 700],
  [OPEN, 3400],
  [BLINK, 110],
];

const SLEEP_SEQUENCE: Array<[string, number]> = SLEEP.map((frame) => [
  frame,
  650,
]);

// While capture runs the eye is open and alive; while it is off the eye
// sleeps. The same grammar as the menu bar icon.
export function AsciiEye({ watching }: { watching: boolean }) {
  const [index, setIndex] = useState(0);
  const sequence = watching ? AWAKE_SEQUENCE : SLEEP_SEQUENCE;

  useEffect(() => {
    setIndex(0);
  }, [watching]);

  useEffect(() => {
    const [, hold] = sequence[index % sequence.length];
    const id = setTimeout(
      () => setIndex((current) => (current + 1) % sequence.length),
      hold,
    );
    return () => clearTimeout(id);
  }, [index, sequence]);

  return (
    <pre className="eye" aria-hidden="true">
      {sequence[index % sequence.length][0]}
    </pre>
  );
}
