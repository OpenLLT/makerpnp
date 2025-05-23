# Known Issues

- [ ] Multiple tasks can be 'started'.  Only one should be 'started/incomplete'.
- [ ] Placement can be placed/skipped before the task is started.
- [ ] Phase ordering is not checked when starting tasks.
- [ ] Changing the design to unit assignment on a PCB which is used by a project that has a design variant assigned to the units with changed designs causes errors.  Workaround: remove assignments from the project before changing the PCB that the project uses.
- [ ] Changes made to a PCB are not shown in the project until the PCB is reloaded.  Workaround: save project, close project, edit pcb, open project. 

## Contributing

If you'd like to fix any of the above issues, pull-requests (PR's) are accepted!