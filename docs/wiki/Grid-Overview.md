### Grid Overview

The Grid Overview is a grid-based view of your windows.
It shows every window as a tile in a grid, letting you quickly navigate between them.

Open it with the `toggle-grid-overview` bind.
You can also bind `open-grid-overview` and `close-grid-overview` separately.

While in the grid overview, all keyboard shortcuts keep working.
Use your normal focus key binds, for example the default Mod+Arrow binds, to move the grid selection.

Press <kbd>Enter</kbd> to activate the selected window and close the grid overview.
Press <kbd>Escape</kbd> to close it.

Click a window to activate it and close the grid overview.
Click empty space to close the grid overview.

Floating windows are included in the grid.
Tabbed columns show each tab as a separate grid item.
Non-tabbed columns show as one grid item, and Up/Down switches between windows inside the column.

Window shadows are not shown in the grid overview.

### Configuration

See the full documentation for the `grid-overview` section [here](./Configuration:-Miscellaneous.md#grid-overview).

You can set the gap between cells like this:

```kdl
grid-overview {
    gap 16
}
```

To change the padding around the grid, use the `padding` setting:

```kdl
grid-overview {
    padding 32
}
```

To change the focused window scale, use the `focused-window-scale` setting:

```kdl
grid-overview {
    focused-window-scale 1.08
}
```

For very wide or very tall windows, `min-scale` controls the minimum size of the shorter preview dimension relative to its grid cell:

```kdl
grid-overview {
    min-scale 0.08
}
```

The open/close, rearrange, and selection scale animation is configured with [`animations.grid-overview-open-close`](./Configuration:-Animations.md#grid-overview-open-close).
