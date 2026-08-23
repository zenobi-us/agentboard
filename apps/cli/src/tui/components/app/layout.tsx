
export function AppLayout(props: {
  header: React.ReactNode;
  footer: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <box flexDirection="column" flexGrow={1}>
      <box flexDirection="row" marginBottom={1} border={false}>
        {props.header}
      </box>
      <box flexDirection="row" border={false} flexGrow={1}>
        {props.children}
      </box>
      <box flexDirection="row" border={false}>
        {props.footer}
      </box>
    </box>
  );
}
