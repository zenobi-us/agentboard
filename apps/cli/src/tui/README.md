# ClankPipe TUI

- xstate machines
- opentui
- beautiful-mermaid

## Views

### App

- renders loader(lg) while loading workspace
- loads workspace
- when loaded. renders workspace view

### Workspace View

- renders source-summary-card for each source in workspace
- clicking on source-summary-card opens source-view


### Source View

- renders source-summary-card for the source
- renders a table of items for the source

### Item Detail View

- renders item-detail for the item... undecided how at the moment. maybe a modal overlay
