# Known Issues

## Planner

### Multiple tasks can be 'started'.

Only one should be 'started/incomplete'.

#### Workaround

Complete one task before starting the next.

### Placement can be placed/skipped before the task is started.

#### Workaround

Don't mark placements as placed/skipped before the task is started.

### Tricky to re-arrange phases by dragging and dropping

Due to usability issues with egui_ltreeview, it's not clear where you can drop
items from the tree views.

#### Workaround

* Close child nodes before attempting to re-arranging siblings.
* Try the operation in reverse, e.g. instead of dragging a middle item below the last item, drag the last item up.

### Phase ordering is not checked when starting tasks.

#### Workaround

Remember to start the tasks in the correct order.

### Changing the design-to-unit assignment on a PCB which is used by a project that has a design variant assigned to the units with changed designs causes errors.  

#### Workaround

remove assignments from the project before changing the PCB that the project uses.

### Changes made to a PCB are not shown in the project until the PCB is reloaded.  

#### Workaround

save project, close project, edit pcb, open project. 

### GUI layout incorrectly persisted/restored.

egui_dock fails to persist/restore certain GUI layouts. 

#### Workaround

Delete the corresponding 'app.ron' file in your user profile directory.

On Windows this is: `%appdata%\MakerPnP - Planner\data\app.ron`.

### The same project or PCB can be opened multiple times.

#### Workaround

Don't open the project or PCB multiple times.

### debug mode panic when un-floating a tab that contains a data table.

To reproduce, run the app in debug mode, pull out the placements tab, then re-insert it.

#### Workaround

Run in release mode, or don't float tabs that contain data tables in debug mode.

#### Bug report

https://github.com/Adanos020/egui_dock/issues/278

# Contributing

The plan is to resolve all of these issues eventually; if you'd like to fix any of the above issues then pull-requests (PR's) are accepted!
