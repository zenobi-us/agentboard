// @ts-expect-error The TUI currently has no local React type package.
import { useEffect, useState } from "react"

type SpinnerSize = "sm" | "lg"
type SpinnerFrame = readonly (readonly string[])[]

const spinnerFrames: Record<SpinnerSize, readonly SpinnerFrame[]> = {
  sm: [
    [["⠋"]],
    [["⠙"]],
    [["⠹"]],
    [["⠸"]],
    [["⠼"]],
    [["⠴"]],
    [["⠦"]],
    [["⠧"]],
    [["⠇"]],
    [["⠏"]],
  ],
  md: [
    [["o", "o"], ["o", " "], [" ", " "]],
    [["o", "o"], [" ", "o"], [" ", " "]],
    [["o", "o"], [" ", "o"], [" ", "o"]],
    [[" ", "o"], [" ", "o"], [" ", "o"]],
    [[" ", "o"], [" ", "o"], ["o", "o"]],
    [[" ", " "], [" ", "o"], ["o", "o"]],
    [[" ", " "], ["o", " "], ["o", "o"]],
    [["o", " "], ["o", " "], ["o", "o"]],
    [["o", " "], ["o", " "], ["o", " "]],
    [["o", "o"], ["o", " "], ["o", " "]],
  ],


  lg: [
    // frame 1
    [
      ["●", "●"],
      ["●", " "],
      [" ", " "],
    ],
    // frame 2
    [
      ["●", "●"],
      [" ", "●"],
      [" ", " "],
    ],
    // frame 3
    [
      ["●", "●"],
      [" ", "●"],
      [" ", "●"],
    ],
    // frame 4
    [
      [" ", "●"],
      [" ", "●"],
      [" ", "●"],
    ],
    // frame 5
    [
      [" ", "●"],
      [" ", "●"],
      ["●", "●"],
    ],
    // frame 6
    [
      [" ", " "],
      [" ", "●"],
      ["●", "●"],
    ],
    // frame 7
    [
      [" ", " "],
      ["●", " "],
      ["●", "●"],
    ],
    // frame 8
    [
      ["●", " "],
      ["●", " "],
      ["●", "●"],
    ],
    // frame 9
    [
      ["●", " "],
      ["●", " "],
      ["●", " "],
    ],
    // frame 10
    [
      ["●", "●"],
      ["●", " "],
      ["●", " "],
    ],
  ],
}

export function Loader(props: { size?: SpinnerSize }) {
  const frames = spinnerFrames[props.size ?? "sm"]
  const [frameIndex, setFrameIndex] = useState(0)

  useEffect(() => {
    const interval = setInterval(() => {
      setFrameIndex((current: number) => (current + 1) % frames.length)
    }, 100)

    return () => clearInterval(interval)
  }, [frames])

  const frame = frames[frameIndex] ?? []

  return (
    <box>
      {frame.map((row, rowIndex) => (
        <box key={rowIndex} flexDirection="row">
          {row.map((cell, cellIndex) => (
            <text key={cellIndex}>{cell}</text>
          ))}
        </box>
      ))}
    </box>
  )
}
