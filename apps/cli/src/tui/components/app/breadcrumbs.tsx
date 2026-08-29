type BreadcrumbItem = {
  label: string
  onClick?: () => void
}

export function Breadcrumbs(props: { items: BreadcrumbItem[] }) {
  return (
    <BreadcrumbRow>
      {props.items.map((item, index) => (
        <>
          <BreadcrumbsItem key={index} onClick={item.onClick}>
            {item.label}
          </BreadcrumbsItem>
          {index < props.items.length - 1 && <BreadcrumbsSeparator />}
        </>
      ))}
    </BreadcrumbRow>
  )
}

function BreadcrumbRow(props: { children: React.ReactNode }) {

  return (
    <box flexDirection="row" marginBottom={1}>
      {props.children}
    </box>
  )
}

function BreadcrumbsItem(props: { onClick?: () => void; children?: React.ReactNode; }) {
  return (
    <box flexDirection="row" onMouseDown={props.onClick}>
      {typeof props.children === "string" ? (
        <text fg={props.onClick ? "blue" : "white"}>{props.children}</text>
      ) : props.children}
    </box>
  )
}

function BreadcrumbsSeparator() {
  return (
    <box flexDirection="row">
      <text> / </text>
    </box>
  )
}

Breadcrumbs.Item = BreadcrumbsItem
Breadcrumbs.Row = BreadcrumbRow
Breadcrumbs.Separator = BreadcrumbsSeparator
