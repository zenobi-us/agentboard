import { useTheme } from "../../services/theme/theme.tsx"

export function Badge(props: {
  children: React.ReactNode
  fg?: string
  bg?: string
}) {
  return (
    <box
      border={false}
      backgroundColor={props.bg}
      flexDirection="row"
      paddingY={0}
    >
      {typeof props.children === "string" ? <text fg={props.fg}>{props.children}</text> : props.children}
    </box>
  )
}

function BadgeType(props: {
  type: "Source" | "Action" | "Item" | "Workspace"
  label: string
}) {
  const theme = useTheme()
  const style = theme.component(`badge.${props.type.toLowerCase()}`)
  const bg = style.bg ?? "#000000"
  const fg = contrastForeground(bg, false)
  const typeBg = mix(bg, contrastForeground(bg, true), 0.35)
  const typeFg = contrastForeground(typeBg, true)
  const typeChar = props.type.charAt(0).toUpperCase()

  return (
    <Badge bg={bg} fg={fg}>
      <box backgroundColor={typeBg}>
        <text fg={typeFg}>{typeChar}</text>
      </box>
      <text fg={fg}> {props.label}</text>
    </Badge>
  )
}

Badge.Type = BadgeType

function contrastForeground(background: string, maximum: boolean): string {
  const rgb = parseHex(background)
  if (!rgb) return "#ffffff"

  const luminance = relativeLuminance(rgb)
  const foreground = luminance > 0.179 ? "#000000" : "#ffffff"
  if (maximum) return foreground
  return mix(background, foreground, 0.9)
}

function parseHex(value: string): [number, number, number] | undefined {
  const match = /^#([\da-f]{3}|[\da-f]{6})$/i.exec(value)
  if (!match) return undefined
  const hex = match[1]!.length === 3
    ? match[1]!.split("").map((part) => part + part).join("")
    : match[1]!
  return [0, 2, 4].map((index) => Number.parseInt(hex.slice(index, index + 2), 16)) as [number, number, number]
}

function relativeLuminance([red, green, blue]: [number, number, number]): number {
  const linear = (channel: number) => {
    const value = channel / 255
    return value <= 0.03928 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4
  }
  return 0.2126 * linear(red) + 0.7152 * linear(green) + 0.0722 * linear(blue)
}

function mix(background: string, foreground: string, amount: number): string {
  const bg = parseHex(background)
  const fg = parseHex(foreground)
  if (!bg || !fg) return foreground
  return `#${bg.map((channel, index) => Math.round(channel + (fg[index]! - channel) * amount).toString(16).padStart(2, "0")).join("")}`
}
