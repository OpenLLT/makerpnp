# Known Issues

## Planner

### Multiple tasks can be 'started'.

Only one should be 'started/incomplete'.

#### Workaround

Complete one task before starting the next.

### Placement can be placed/skipped before the task is started.

#### Workaround

Don't mark placements as placed/skipped before the task is started.

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

# Contributing

The plan is to resolve all of these issues eventually; if you'd like to fix any of the above issues then pull-requests (PR's) are accepted!
