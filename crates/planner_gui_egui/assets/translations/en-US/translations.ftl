theme-button-light = ☀ Light
theme-button-dark = 🌙 Dark
theme-button-system = 💻 System

# format "<language in native language> (<country in native language>)
language-es-ES = Español (España)
language-en-US = English (United States)

menu-top-level-file = File
menu-item-quit = Quit

modal-errors-title = Errors - { $file }
modal-add-pcb-title = Add PCB - { $file }
modal-add-phase-title = Add phase - { $file }
modal-create-unit-assignment-title = Create unit assignment - { $file }
modal-phase-placement-orderings-title = Phase placement orderings - { $phase }
modal-manager-gerbers-title = Manage gerbers - { $design }

toolbar-button-home = Home
toolbar-button-new = New
toolbar-button-open = Open
toolbar-button-save = Save
toolbar-button-close-all = Close all

side-bar-header = Sidebar header
side-bar-footer = Sidebar footer
side-bar-item-path = Path

project-toolbar-button-show-explorer = Show explorer
project-toolbar-button-generate-artifacts = Generate artifacts
project-toolbar-button-refresh-from-variants = Refresh from variants
project-toolbar-button-remove-unused-placements = Remove unused placements
project-toolbar-button-add-pcb = Add PCB
project-toolbar-button-add-phase = Add phase
project-toolbar-button-create-unit-assignment = Create unit assignment

tab-label-home = Home
tab-label-new-project = New project

home-banner = MakerPnP - Planner
home-checkbox-label-show-on-startup = Show on startup

new-project-banner = New project
form-new-project-input-name = Project name
form-new-project-input-directory = Directory

project-detail-path = Path: { $path }
project-detail-name = Name: { $name }

project-overview-tab-label = Overview
project-overview-header = Overview
project-overview-detail-name = Name: { $name }

project-pcb-header = PCB
project-pcb-designs-header = Designs
project-pcb-designs-button-gerbers = Gerbers...

project-placements-tab-label = Placements
project-placements-header = Project placements

project-parts-tab-label = Parts
project-parts-header = Project Parts

project-pcb-tab-label = PCB ({ $pcb })

project-unit-assignments-tab-label = Unit assignments ({ $pcb })
project-unit-assignments-header = Unit assignments

phase-toolbar-add-parts-to-loadout = Add parts to load-out.
phase-toolbar-placement-orderings = Placement orderings.

phase-placements-header = Phase Placements

phase-properties-header = Phase properties
phase-properties-footer = { $count } items

phase-reference = Reference
phase-load-out-source = Load-out source
phase-pcb-side = PCB Side
phase-process = Process

project-load-out-header = Load-out

table-load-out-column-reference = Reference
table-load-out-column-manufacturer = Manufacturer
table-load-out-column-mpn = MPN

pcb-side-top = Top
pcb-side-bottom = Bottom
pcb-side-both = Both

project-explorer-tab-label = Project explorer

project-explorer-node-root = Root
project-explorer-node-parts = Parts
project-explorer-node-placements = Placements
project-explorer-node-phases = Phases
project-explorer-node-phase = { $reference } ({ $process})
project-explorer-node-phase-loadout = { $source }
project-explorer-node-unit-assignments = Unit assignments
project-explorer-node-unit-assignment-assigned = { $name } {$design_name} = {$variant_name}
project-explorer-node-unit-assignment-unassigned = { $name } {$design_name} = Unassigned
project-explorer-node-pcbs = PCBs
project-explorer-node-pcb = { $name }
project-explorer-node-processes = Processes
project-explorer-node-process = { $name }

form-button-ok = Ok
form-button-cancel = Cancel
form-button-close = Close
form-button-add = Add
form-button-remove = Remove

form-add-pcb-input-name = PCB Name
form-add-pcb-input-name-placeholder = (e.g. the PCB factory's order reference number)
form-add-pcb-input-units = Units
form-add-pcb-input-design-name = Design name
form-add-pcb-input-design-name-placeholder = (e.g. 'my_eda_project', for unit assignments)
form-add-pcb-unit-map = Unit map
form-add-pcb-assign-selection = Assign to selection
form-add-pcb-assign-all = Assign all
form-add-pcb-unassign-selection = Unasign selection
form-add-pcb-unassign-all = Unasign all

form-create-unit-assignment-group-variant-map = Variant map
form-create-unit-assignment-input-design-name = Design name
form-create-unit-assignment-input-design-name-placeholder = Design name (e.g. 'my design')
form-create-unit-assignment-input-variant-name = Variant name
form-create-unit-assignment-input-variant-name-placeholder = Variant name (e.g. 'default')
form-create-unit-assignment-input-pcb-instance = PCB instance
form-create-unit-assignment-input-pcb-instance-placeholder = A number > 0
form-create-unit-assignment-input-pcb-unit-range = PCB units
form-create-unit-assignment-input-pcb-unit-placeholder = A number > 0
form-create-unit-assignment-input-placements-filename = Placements filename
form-create-unit-assignment-input-placements-filename-placeholder = '<design>_<variant>_placements.csv'
form-create-unit-assignment-input-placements-directory = Placements directory

form-phase-placement-orderings-input-orderings = Orderings

form-common-combo-select = Select...
form-common-combo-none = None

form-common-choice-pcb-kind = PCB Kind
form-common-choice-pcb-kind-single = Single
form-common-choice-pcb-kind-panel = Panel

form-common-choice-pcb-side = PCB Side
form-common-choice-pcb-side-top = Top
form-common-choice-pcb-side-bottom = Bottom

form-common-input-load-out-source = Loadout source
form-common-input-phase-reference = Phase reference
form-common-choice-process = Process

form-common-button-assign-selected = Assign selected
form-common-button-unassign-selected = Unassign selected
form-common-button-unassign-all = Assign all
form-common-button-unassign-range = Unassign range

form-common-button-apply-range = Apply range
form-common-button-apply-all = Apply all

form-common-button-add = Add
form-common-button-remove = Remove
form-common-button-delete = Delete

form-option-error-required = * Required

form-input-error-empty = Cannot be empty
form-input-error-length = Minimum length { $min }
form-input-error-range = Out of range, required range: { $min } - { $max } (inclusive)
form-choice-empty = Choose an option

form-input-number-require-greater-than-zero = Require a number greater than zero
form-input-number-require-positive-number = Require a number
form-file-not-found = File not found

form-input-error-map-incorrect-entry-count = Incorrect entry count.  Required: { $required }, Actual: { $actual }
form-input-error-map-unassigned-entries = Mapping contains unassigned entries.

assignment-assigned = Assigned
assignment-unassigned = Unassigned

placement-placed = Placed
placement-pending = Pending
placement-skipped = Skipped

placement-place = Place
placement-no-place = No-place

placement-project-status-used = Used
placement-project-status-unused = Unused

sort-mode-feeder-reference = Feeder reference
sort-mode-pcb-unit = PCB unit
sort-mode-ref-des = Ref. Des.

sort-order-ascending = Ascending
sort-order-descending = Descending

process-status-pending = Pending
process-status-incomplete = Incomplete
process-status-complete = Complete
process-status-abandoned = Abandoned

gerber-purpose-assembly = Assembly
gerber-purpose-copper = Copper
gerber-purpose-legend = Legend
gerber-purpose-pastemask = Paste mask
gerber-purpose-other = Other
gerber-purpose-soldermask = Soler mask

table-placements-column-index = #
table-placements-column-object-path = Object path
table-placements-column-refdes = Ref. Des.
table-placements-column-place = Place
table-placements-column-placed = Placed?
table-placements-column-manufacturer = Manufacturer
table-placements-column-mpn = MPN
table-placements-column-rotation = Rotation
table-placements-column-x = X
table-placements-column-y = Y
table-placements-column-pcb-side = Side
table-placements-column-phase = Phase
table-placements-column-status = Status
table-placements-column-ordering = Ordering

table-parts-column-index = #
table-parts-column-manufacturer = Manufacturer
table-parts-column-mpn = MPN
table-parts-column-processes = Processes
table-parts-column-ref-des-set = Ref. Des. Set
table-parts-column-quantity = Quantity

table-designs-column-index = #
table-designs-column-actions = Actions
table-designs-column-name = Name

table-gerbers-column-index = #
table-gerbers-column-file = File
table-gerbers-column-pcb-side = PCB Side
table-gerbers-column-gerber-purpose = Purpose
table-gerbers-column-actions = Actions

table-design-variants-column-design = Design name
table-design-variants-column-variant = Variant name

table-unit-assignments-column-pcb-unit = PCB Unit
table-unit-assignments-column-design = Design name
table-unit-assignments-column-variant = Variant name

filter-expression = Search...

expanding-header-details = Details

#
# egui-data-tables
#

# cell context menu
context-menu-selection-copy = Selection: Copy
context-menu-selection-cut = Selection: Cut
context-menu-selection-clear = Selection: Clear
context-menu-selection-fill = Selection: Fill
context-menu-clipboard-paste = Clipboard: Paste
context-menu-clipboard-insert = Clipboard: Insert
context-menu-row-duplicate = Row: Duplicate
context-menu-row-delete = Row: Delete
context-menu-undo = Undo
context-menu-redo = Redo
# column header context menu
context-menu-hide = Hide column
context-menu-hidden = Hidden columns
context-menu-clear-sort = Clear sorting
