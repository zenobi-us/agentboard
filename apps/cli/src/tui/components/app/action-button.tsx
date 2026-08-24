import { useTheme } from "../../services/theme/theme.tsx"
import { Loader } from "./loader.tsx"

export function ActionButton(props: {
  label: string
  shortcut: string
  themeToken: "button.run" | "button.watch"
  loading?: boolean
  disabled?: boolean
  onPress: () => void
}) {
  const theme = useTheme()
  const style = theme.component(
    props.disabled ? `${props.themeToken}.disabled` : props.loading ? `${props.themeToken}.busy` : props.themeToken,
  )

  return (
    <box
      flexDirection="row"
      border={false}
      marginRight={1}
      paddingX={1}
      backgroundColor={style.bg}
      opacity={props.disabled ? 0.5 : 1}
      onMouseDown={props.disabled ? undefined : props.onPress}
    >
      <box flexDirection="row" marginRight={1}>
        {props.loading ? <Loader size="sm" fg={style.fg} /> : <text fg={style.fg}>{props.shortcut}</text>}
      </box>
      <text fg={style.fg}>{props.label}</text>
    </box>
  )
}
