# Hacks

These hacks can be found in the codebase, and the same pattern is used everywhere the corresponding HACK comment is
found.

## table-resize-hack

```
HACK: table-resize-hack - this hack adds a frame around the table using `egui::Frame`, without the frame there are issues:
      1) the bottom row of the table extends outside the boundary of the `Resize` frame.
      2) the resize handle doesn't work from the bottom right corner due to the scrollbars.
```

example:

```rust
Resize::default()
    // ...
    .show(ui, | ui| {
        egui::Frame::new()        // <-- shouldn't need this frame
            .outer_margin(4.0)    // <-- shouldn't need this margin
            .show(ui, |ui | {
                // ...
                TableBuilder::new(ui)
                    // ...
                    .body( |mut body | {
                    // ...
                    });
            });
    });
```
