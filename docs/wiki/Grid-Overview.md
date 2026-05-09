### Grid Overview

The Grid Overview is a grid-based view of your windows.
It shows every window as a tile in a grid, letting you quickly navigate between them.

Open it with the `toggle-grid-overview` bind.

While in the grid overview, all keyboard shortcuts keep working.
Use the arrow keys to navigate between windows.
If a column contains multiple windows, use Up/Down to switch between them within the same cell.

Click a window to activate it and close the grid overview.

### Configuration

See the full documentation for the `grid-overview` section [here](./Configuration:-Miscellaneous.md#grid-overview).

You can set the gap between cells like this:

```kdl
grid-overview {
    gap 16
}
```

To change the focused column scale, use the `focused-column-scale` setting:

```kdl
grid-overview {
    focused-column-scale 1.08
}
```
